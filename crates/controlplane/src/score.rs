use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Thresholds {
    pub error_rise_pp: f64,
    pub p95_factor: f64,
    pub p95_margin_ms: f64,
    pub healthy_error_pp: f64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub error_pp: f64,
    pub p95_ms: f64,
    pub restarts: f64,
    pub unready: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Target,
    Clean,
    Impacted,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Delta {
    pub tenant: String,
    pub role: Role,
    pub baseline: Option<Window>,
    pub inject: Option<Window>,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    pub invalid: Option<String>,
    pub blast_radius: Option<usize>,
    pub tenants: Vec<Delta>,
}

impl Thresholds {
    pub fn moved(&self, baseline: &Window, now: &Window) -> Vec<String> {
        let mut reasons = Vec::new();
        let rise = now.error_pp - baseline.error_pp;
        if rise > self.error_rise_pp {
            reasons.push(format!("error ratio +{rise:.2} pp"));
        }
        if now.p95_ms > baseline.p95_ms * self.p95_factor
            && now.p95_ms > baseline.p95_ms + self.p95_margin_ms
        {
            reasons.push(format!("p95 {:.0} → {:.0} ms", baseline.p95_ms, now.p95_ms));
        }
        if now.restarts > 0.0 {
            reasons.push(format!("{} restarts", now.restarts));
        }
        if now.unready > 0.0 {
            reasons.push(format!("{} unready", now.unready));
        }
        reasons
    }

    fn unhealthy(&self, name: &str, baseline: Option<&Window>) -> Option<String> {
        let Some(b) = baseline else {
            return Some(format!("{name} had no traffic in the baseline"));
        };
        if b.error_pp > self.healthy_error_pp {
            return Some(format!("{name} baseline error ratio {:.2}%", b.error_pp));
        }
        (b.restarts > 0.0 || b.unready > 0.0)
            .then(|| format!("{name} restarted or was unready in the baseline"))
    }
}

pub fn score(
    t: &Thresholds,
    target: &str,
    tenants: &[String],
    baseline: &BTreeMap<String, Window>,
    inject: &BTreeMap<String, Window>,
) -> Score {
    let invalid = tenants
        .iter()
        .find_map(|name| t.unhealthy(name, baseline.get(name)));
    let deltas: Vec<Delta> = tenants
        .iter()
        .map(|name| {
            let (b, i) = (baseline.get(name).copied(), inject.get(name).copied());
            let reasons = match (name == target, b, i) {
                (true, _, _) => Vec::new(),
                (false, Some(b), Some(i)) => t.moved(&b, &i),
                (false, _, None) => vec!["no traffic while the fault was active".into()],
                (false, None, Some(_)) => Vec::new(),
            };
            let role = match (name == target, reasons.is_empty()) {
                (true, _) => Role::Target,
                (false, true) => Role::Clean,
                (false, false) => Role::Impacted,
            };
            Delta {
                tenant: name.clone(),
                role,
                baseline: b,
                inject: i,
                reasons,
            }
        })
        .collect();
    let impacted = deltas.iter().filter(|d| d.role == Role::Impacted).count();
    Score {
        blast_radius: invalid.is_none().then_some(impacted),
        invalid,
        tenants: deltas,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Thresholds = Thresholds {
        error_rise_pp: 0.5,
        p95_factor: 1.5,
        p95_margin_ms: 10.0,
        healthy_error_pp: 1.0,
    };

    fn w(error_pp: f64, p95_ms: f64) -> Window {
        Window {
            error_pp,
            p95_ms,
            ..Window::default()
        }
    }

    fn run(baseline: &[(&str, Window)], inject: &[(&str, Window)]) -> Score {
        let map = |s: &[(&str, Window)]| s.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        let tenants = ["acme", "globex", "initech"].map(String::from);
        score(&T, "acme", &tenants, &map(baseline), &map(inject))
    }

    fn healthy() -> Vec<(&'static str, Window)> {
        vec![
            ("acme", w(0.0, 20.0)),
            ("globex", w(0.1, 20.0)),
            ("initech", w(0.0, 4.0)),
        ]
    }

    #[test]
    fn target_damage_does_not_count_and_quiet_bystanders_are_clean() {
        let mut inject = healthy();
        inject[0].1 = Window {
            error_pp: 50.0,
            p95_ms: 900.0,
            restarts: 3.0,
            unready: 1.0,
        };
        let s = run(&healthy(), &inject);
        assert_eq!(s.blast_radius, Some(0));
        assert_eq!(s.tenants[0].role, Role::Target);
        assert_eq!(s.tenants[1].role, Role::Clean);
    }

    #[test]
    fn error_rise_counts_only_above_half_a_point() {
        let mut inject = healthy();
        inject[1].1 = w(0.55, 20.0);
        assert_eq!(run(&healthy(), &inject).blast_radius, Some(0));
        inject[1].1 = w(0.7, 20.0);
        let s = run(&healthy(), &inject);
        assert_eq!(s.blast_radius, Some(1));
        assert_eq!(s.tenants[1].reasons, ["error ratio +0.60 pp"]);
    }

    #[test]
    fn latency_needs_both_the_factor_and_the_margin() {
        let mut inject = healthy();
        inject[2].1 = w(0.0, 9.0);
        assert_eq!(run(&healthy(), &inject).blast_radius, Some(0));
        inject[1].1 = w(0.1, 29.0);
        assert_eq!(run(&healthy(), &inject).blast_radius, Some(0));
        inject[1].1 = w(0.1, 31.0);
        inject[2].1 = w(0.0, 15.0);
        let s = run(&healthy(), &inject);
        assert_eq!(s.blast_radius, Some(2));
        assert_eq!(s.tenants[2].reasons, ["p95 4 → 15 ms"]);
    }

    #[test]
    fn restarts_unready_and_silence_impact_a_bystander() {
        let mut inject = healthy();
        inject[1].1.restarts = 1.0;
        inject.remove(2);
        let s = run(&healthy(), &inject);
        assert_eq!(s.blast_radius, Some(2));
        assert_eq!(s.tenants[1].reasons, ["1 restarts"]);
        assert_eq!(
            s.tenants[2].reasons,
            ["no traffic while the fault was active"]
        );
    }

    #[test]
    fn unhealthy_or_missing_baseline_makes_the_run_invalid_but_keeps_the_deltas() {
        let mut baseline = healthy();
        baseline[1].1 = w(2.0, 20.0);
        let s = run(&baseline, &healthy());
        assert_eq!(s.blast_radius, None);
        assert_eq!(
            s.invalid.as_deref(),
            Some("globex baseline error ratio 2.00%")
        );
        assert_eq!(s.tenants.len(), 3);

        baseline = healthy();
        baseline.remove(2);
        let s = run(&baseline, &healthy());
        assert_eq!(
            s.invalid.as_deref(),
            Some("initech had no traffic in the baseline")
        );

        baseline = healthy();
        baseline[0].1.unready = 1.0;
        assert!(run(&baseline, &healthy()).invalid.is_some());
    }
}
