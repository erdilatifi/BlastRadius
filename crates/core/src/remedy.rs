use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use crate::{Amount, GATE_PERCENT, Image, Tenant, Tier};

pub const RELEASE_WINDOW: i64 = 15 * 60;
pub const OOM_KILLS: f64 = 3.0;
pub const HOURLY_CAP: usize = 3;

#[derive(Debug, PartialEq)]
pub enum Playbook {
    RevertRelease,
    UpgradeTier,
}

#[derive(Debug)]
pub struct Proposal {
    pub title: String,
    pub body: String,
    pub values: String,
}

pub fn choose(alert: &str, release_age: Option<i64>, oom_kills: f64) -> Option<Playbook> {
    let burning = alert.ends_with("FastBurn") || alert == "TenantCrashLooping";
    if burning && release_age.is_some_and(|age| (0..=RELEASE_WINDOW).contains(&age)) {
        return Some(Playbook::RevertRelease);
    }
    (oom_kills >= OOM_KILLS).then_some(Playbook::UpgradeTier)
}

pub fn guard(tenant: &Tenant, open_branches: &[String], opened_last_hour: usize) -> Result<()> {
    let name = &tenant.name;
    ensure!(tenant.remediation, "{name} has remediation: false");
    let prefix = format!("remediate-{name}-");
    ensure!(!open_branches.iter().any(|b| b.starts_with(&prefix)), "a remediation PR for {name} is already open");
    ensure!(opened_last_hour < HOURLY_CAP, "the cap of {HOURLY_CAP} remediation PRs per hour is reached");
    Ok(())
}

pub fn revert(now: &Tenant, previous: &str, commit: &str) -> Result<Proposal> {
    let (name, bad, minutes) = (&now.name, &now.image.tag, RELEASE_WINDOW / 60);
    ensure!(bad != previous, "{name} already runs {previous}");
    let fixed = Tenant { image: Image { tag: previous.into() }, ..now.clone() };
    Ok(Proposal {
        title: format!("Revert {name} to {previous}"),
        body: format!(
            "Playbook `revert-release`. {name} started burning its error budget within {minutes} minutes of {commit}, \
             which moved the image from `{previous}` to `{bad}`. Merging sets it back; closing keeps the release."
        ),
        values: fixed.to_yaml()?,
    })
}

pub fn upgrade(now: &Tenant, tiers: &BTreeMap<String, Tier>, capacity: Amount, others: Amount) -> Result<Proposal> {
    let (name, from) = (&now.name, &now.tier);
    let current = tiers.get(from).with_context(|| format!("unknown tier {from}"))?;
    let bigger = tiers.iter().filter(|(_, t)| t.quota.memory_mib > current.quota.memory_mib);
    let (next, tier) =
        bigger.min_by_key(|(_, t)| t.quota.memory_mib).context(format!("{name} is on the largest tier"))?;
    ensure!(capacity.fits(others, tier.quota) > 0, "{name} on {next} does not fit under {GATE_PERCENT}% of capacity");
    let after = others.plus(tier.quota).fields().into_iter().zip(capacity.fields());
    let shares: Vec<String> =
        after.map(|((what, used), (_, cap))| format!("{what} {}%", used * 100 / cap.max(1))).collect();
    let (old, new) = (current.limits.memory_mib, tier.limits.memory_mib);
    Ok(Proposal {
        title: format!("Move {name} from {from} to {next}"),
        body: format!(
            "Playbook `upgrade-tier`. Containers were OOM-killed at the {from} memory limit of {old} MiB; {next} allows {new} MiB.\n\nCommitted after this change: {} of capacity (gate {GATE_PERCENT}%).",
            shares.join(", ")
        ),
        values: Tenant { tier: next.clone(), ..now.clone() }.to_yaml()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiers() -> BTreeMap<String, Tier> {
        let tier = |cpu: u64, mem: u64, pods: u64| Tier {
            requests: Amount { cpu_millis: cpu / 8, memory_mib: mem / 8, pods: 0 },
            limits: Amount { cpu_millis: cpu / 2, memory_mib: mem / 4, pods: 0 },
            quota: Amount { cpu_millis: cpu, memory_mib: mem, pods },
        };
        BTreeMap::from([
            ("small".into(), tier(400, 256, 6)),
            ("medium".into(), tier(800, 512, 8)),
            ("large".into(), tier(1600, 1024, 12)),
        ])
    }

    const CAPACITY: Amount = Amount { cpu_millis: 4000, memory_mib: 4096, pods: 60 };

    #[test]
    fn matches_a_burn_after_a_release_and_repeated_oom_kills() {
        assert_eq!(choose("TenantAvailabilityFastBurn", Some(300), 0.0), Some(Playbook::RevertRelease));
        assert_eq!(choose("TenantCrashLooping", Some(0), 0.0), Some(Playbook::RevertRelease));
        assert_eq!(choose("TenantAvailabilityFastBurn", Some(3600), 0.0), None);
        assert_eq!(choose("TenantAvailabilitySlowBurn", Some(60), 0.0), None);
        assert_eq!(choose("TenantAvailabilityFastBurn", None, 3.0), Some(Playbook::UpgradeTier));
        assert_eq!(choose("TenantLatencySlowBurn", None, 2.0), None);
    }

    #[test]
    fn reverts_the_image_tag_only() {
        let now = Tenant::new("acme", "small", "sha-bad0000");
        let p = revert(&now, "sha-good000", "abc1234").unwrap();
        let fixed: Tenant = serde_saphyr::from_str(&p.values).unwrap();
        assert_eq!(fixed, Tenant { image: crate::Image { tag: "sha-good000".into() }, ..now.clone() });
        assert!(p.body.contains("abc1234"));
        assert!(revert(&now, "sha-bad0000", "abc1234").is_err());
    }

    #[test]
    fn upgrades_to_the_next_tier_when_it_fits() {
        let p = upgrade(&Tenant::new("acme", "small", "sha-1"), &tiers(), CAPACITY, Amount::default()).unwrap();
        let fixed: Tenant = serde_saphyr::from_str(&p.values).unwrap();
        assert_eq!(fixed.tier, "medium");
        assert!(p.body.contains("cpu 20%, memory 12%, pods 13%"));
    }

    #[test]
    fn opens_nothing_when_the_tier_does_not_fit() {
        let full = Amount { cpu_millis: 3000, memory_mib: 1024, pods: 10 };
        let err = upgrade(&Tenant::new("acme", "small", "sha-1"), &tiers(), CAPACITY, full).unwrap_err();
        assert!(err.to_string().contains("does not fit"));
        assert!(upgrade(&Tenant::new("acme", "large", "sha-1"), &tiers(), CAPACITY, Amount::default()).is_err());
    }

    #[test]
    fn guardrails_hold() {
        let mut acme = Tenant::new("acme", "small", "sha-1");
        assert!(guard(&acme, &["remediate-globex-1".into()], 0).is_ok());
        assert!(guard(&acme, &["remediate-acme-1".into()], 0).is_err());
        assert!(guard(&acme, &[], HOURLY_CAP).is_err());
        acme.remediation = false;
        assert!(guard(&acme, &[], 0).is_err());
    }
}
