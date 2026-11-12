use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

const GATE_PERCENT: u64 = 85;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tenant {
    pub name: String,
    pub tier: String,
    pub image: Image,
    pub replicas: Replicas,
    pub slo: Slo,
    pub chaos: bool,
    pub remediation: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub tag: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replicas {
    pub min: u64,
    pub max: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Slo {
    pub availability: f64,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Amount {
    pub cpu_millis: u64,
    #[serde(rename = "memoryMiB")]
    pub memory_mib: u64,
    #[serde(default)]
    pub pods: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tier {
    pub requests: Amount,
    pub limits: Amount,
    pub quota: Amount,
}

#[derive(Deserialize)]
struct ChartValues {
    tiers: BTreeMap<String, Tier>,
}

#[derive(Deserialize)]
struct AppsValues {
    #[serde(rename = "repoURL")]
    repo_url: String,
}

#[derive(Debug, Clone)]
pub struct Repo {
    pub slug: String,
    pub tiers: BTreeMap<String, Tier>,
    pub capacity: Amount,
    pub tenants: Vec<Tenant>,
}

impl Amount {
    fn add(self, o: Amount) -> Amount {
        Amount {
            cpu_millis: self.cpu_millis + o.cpu_millis,
            memory_mib: self.memory_mib + o.memory_mib,
            pods: self.pods + o.pods,
        }
    }

    fn fields(self) -> [(&'static str, u64); 3] {
        [
            ("cpu", self.cpu_millis),
            ("memory", self.memory_mib),
            ("pods", self.pods),
        ]
    }
}

impl Tenant {
    pub fn new(name: &str, tier: &str, tag: &str) -> Tenant {
        Tenant {
            name: name.into(),
            tier: tier.into(),
            image: Image { tag: tag.into() },
            replicas: Replicas { min: 2, max: 4 },
            slo: Slo {
                availability: 99.5,
                latency_ms: 300,
            },
            chaos: true,
            remediation: true,
        }
    }

    pub fn validate(&self, tiers: &BTreeMap<String, Tier>) -> Result<()> {
        let n = &self.name;
        let valid = (2..=20).contains(&n.len())
            && n.starts_with(|c: char| c.is_ascii_lowercase())
            && !n.ends_with('-')
            && n.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        ensure!(
            valid,
            "tenant name {n:?} must be 2-20 of a-z, 0-9, '-', start with a letter, not end with '-'"
        );
        let Some(tier) = tiers.get(&self.tier) else {
            bail!(
                "{n}: unknown tier {:?}, expected one of {:?}",
                self.tier,
                tiers.keys().collect::<Vec<_>>()
            );
        };
        ensure!(
            !self.image.tag.is_empty() && self.image.tag != "latest",
            "{n}: image tag must be pinned, not empty or latest"
        );
        let r = &self.replicas;
        ensure!(
            1 <= r.min && r.min <= r.max,
            "{n}: replicas need 1 <= min <= max"
        );
        let peak = r.max + 1;
        let need = [
            (
                "cpu",
                peak * tier.requests.cpu_millis,
                tier.quota.cpu_millis,
            ),
            (
                "memory",
                peak * tier.requests.memory_mib,
                tier.quota.memory_mib,
            ),
            ("pods", peak, tier.quota.pods),
        ];
        for (what, used, quota) in need {
            ensure!(
                used <= quota,
                "{n}: max {} replicas plus one surge pod need {used} {what}, tier {} allows {quota}",
                r.max,
                self.tier
            );
        }
        let a = self.slo.availability;
        ensure!(
            a > 0.0 && a < 100.0,
            "{n}: slo availability must be between 0 and 100"
        );
        ensure!(
            self.slo.latency_ms > 0,
            "{n}: slo latencyMs must be positive"
        );
        Ok(())
    }

    pub fn to_yaml(&self) -> Result<String> {
        Ok(serde_saphyr::to_string(self)?)
    }
}

fn read<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_saphyr::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

impl Repo {
    pub fn load(root: &Path) -> Result<Repo> {
        let chart: ChartValues = read(&root.join("charts/tenant/values.yaml"))?;
        let apps: AppsValues = read(&root.join("platform/apps/values.yaml"))?;
        let slug = apps
            .repo_url
            .trim_start_matches("https://github.com/")
            .trim_end_matches(".git")
            .to_string();
        let mut dirs: Vec<_> = fs::read_dir(root.join("tenants"))?.collect::<Result<_, _>>()?;
        dirs.sort_by_key(|d| d.file_name());
        let mut tenants = Vec::new();
        for dir in dirs {
            let tenant: Tenant = read(&dir.path().join("values.yaml"))?;
            ensure!(
                *tenant.name == *dir.file_name(),
                "tenants/{}: name {:?} must match the directory",
                dir.file_name().display(),
                tenant.name
            );
            tenants.push(tenant);
        }
        Ok(Repo {
            slug,
            tiers: chart.tiers,
            capacity: read(&root.join("platform/capacity.yaml"))?,
            tenants,
        })
    }

    pub fn tag(&self) -> Result<&str> {
        self.tenants
            .first()
            .map(|t| t.image.tag.as_str())
            .context("no tenant to copy the image tag from")
    }

    pub fn committed(&self) -> Amount {
        self.tenants
            .iter()
            .filter_map(|t| self.tiers.get(&t.tier))
            .fold(Amount::default(), |s, t| s.add(t.quota))
    }

    pub fn check(&self) -> Result<()> {
        for t in &self.tenants {
            t.validate(&self.tiers)?;
        }
        let committed = self.committed();
        for ((what, used), (_, cap)) in committed.fields().into_iter().zip(self.capacity.fields()) {
            ensure!(
                used * 100 <= cap * GATE_PERCENT,
                "capacity gate: {used} {what} committed would exceed {GATE_PERCENT}% of {cap} declared in platform/capacity.yaml"
            );
        }
        Ok(())
    }

    pub fn add(&mut self, tenant: Tenant) -> Result<()> {
        ensure!(
            self.tenants.iter().all(|t| t.name != tenant.name),
            "tenant {} already exists",
            tenant.name
        );
        self.tenants.push(tenant);
        self.check()
    }

    pub fn remove(&mut self, name: &str) -> Result<()> {
        let before = self.tenants.len();
        self.tenants.retain(|t| t.name != name);
        ensure!(self.tenants.len() < before, "tenant {name} does not exist");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> Repo {
        let mut r = Repo::load(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))).unwrap();
        r.tenants.retain(|t| t.name == "acme" || t.name == "globex");
        r
    }

    fn err(repo: &Repo, t: Tenant) -> String {
        let mut r = repo.clone();
        r.add(t).unwrap_err().to_string()
    }

    #[test]
    fn committed_tenants_pass() {
        let r = Repo::load(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))).unwrap();
        r.check().unwrap();
        assert_eq!(r.slug, "erdilatifi/BlastRadius");
    }

    #[test]
    fn round_trips() {
        let t = Tenant::new("initech", "small", "sha-1");
        assert_eq!(
            serde_saphyr::from_str::<Tenant>(&t.to_yaml().unwrap()).unwrap(),
            t
        );
    }

    #[test]
    fn rejects_bad_tenants() {
        let r = repo();
        for name in [
            "a",
            "Acme",
            "1acme",
            "acme-",
            "acme_corp",
            "abcdefghijklmnopqrstu",
        ] {
            assert!(
                err(&r, Tenant::new(name, "small", "sha-1")).contains("tenant name"),
                "{name}"
            );
        }
        assert!(err(&r, Tenant::new("acme", "small", "sha-1")).contains("already exists"));
        assert!(err(&r, Tenant::new("initech", "huge", "sha-1")).contains("unknown tier"));
        assert!(err(&r, Tenant::new("initech", "small", "latest")).contains("pinned"));
        let mut t = Tenant::new("initech", "small", "sha-1");
        t.replicas.max = 9;
        assert!(err(&r, t.clone()).contains("surge"));
        t.replicas = Replicas { min: 3, max: 2 };
        assert!(err(&r, t.clone()).contains("min <= max"));
        t.replicas.min = 1;
        t.slo.availability = 100.0;
        assert!(err(&r, t).contains("availability"));
    }

    #[test]
    fn capacity_gate_refuses_at_85_percent() {
        let mut r = repo();
        r.capacity = Amount {
            cpu_millis: 3000,
            memory_mib: 4096,
            pods: 60,
        };
        r.add(Tenant::new("initech", "small", "sha-1")).unwrap();
        let e = err(&r, Tenant::new("hooli", "large", "sha-1"));
        assert_eq!(
            e,
            "capacity gate: 3200 cpu committed would exceed 85% of 3000 declared in platform/capacity.yaml"
        );
    }

    #[test]
    fn removes_only_existing() {
        let mut r = repo();
        r.remove("acme").unwrap();
        assert!(r.remove("acme").is_err());
    }
}
