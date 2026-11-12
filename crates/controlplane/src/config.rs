use std::collections::BTreeMap;

use anyhow::{Result, ensure};
use serde::Deserialize;

/// Which namespaces are tenants and how to measure them.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tenants {
    /// Kubernetes label selector; every namespace it matches is one tenant.
    pub selector: String,
    /// Label holding the tenant name; the namespace name when unset.
    #[serde(default)]
    pub name_label: Option<String>,
    #[serde(default)]
    pub tier_label: Option<String>,
    #[serde(default)]
    pub opt_in: OptIn,
    /// Pod labels a fault selects inside the target namespace; every pod when empty.
    #[serde(default)]
    pub target_pods: BTreeMap<String, String>,
    pub slis: Slis,
    #[serde(default)]
    pub overrides: BTreeMap<String, SliOverride>,
}

/// The namespace annotation a tenant must carry before chaos may target it.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OptIn {
    pub annotation: String,
    pub value: String,
}

impl Default for OptIn {
    fn default() -> OptIn {
        OptIn { annotation: "chaos-mesh.org/inject".into(), value: "enabled".into() }
    }
}

/// PromQL templates, evaluated once per tenant. Each must return a single series.
/// `$tenant`, `$namespace` and `$range` are replaced before the query is sent.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Slis {
    /// Requests per second.
    pub requests: String,
    /// Failed requests per second; no series means no failures.
    pub errors: String,
    /// 95th percentile latency in seconds.
    pub p95_seconds: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SliOverride {
    requests: Option<String>,
    errors: Option<String>,
    p95_seconds: Option<String>,
}

/// Optional extras the demo environment turns on.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Integrations {
    /// Sync and health from ArgoCD Applications named `<appPrefix><tenant>`.
    pub argocd: Option<ArgoCd>,
    /// Synthetic requests to every tenant, so SLIs have traffic to measure.
    pub traffic: Option<Traffic>,
    /// PromQL that returns a series while the target's alert fires; gives time to detect.
    pub detect_alert: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArgoCd {
    pub namespace: String,
    /// Applications are named `<appPrefix><tenant>`; empty means just the tenant name.
    #[serde(default)]
    pub app_prefix: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Traffic {
    pub url: String,
    /// Host header, `$tenant` is replaced.
    pub host: String,
    pub path: String,
}

pub fn fill(template: &str, tenant: &str, namespace: &str, range: &str) -> String {
    template.replace("$tenant", tenant).replace("$namespace", namespace).replace("$range", range)
}

impl Tenants {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.selector.trim().is_empty(), "tenants.selector must not be empty");
        for (key, query) in self.slis.entries() {
            ensure!(!query.trim().is_empty(), "tenants.slis.{key} must not be empty");
            ensure!(
                query.contains("$namespace") || query.contains("$tenant"),
                "tenants.slis.{key} must use $namespace or $tenant, or every tenant gets the same value"
            );
            ensure!(query.contains("$range"), "tenants.slis.{key} must use $range so it covers the run's window");
        }
        for (tenant, o) in &self.overrides {
            for (key, query) in [("requests", &o.requests), ("errors", &o.errors), ("p95Seconds", &o.p95_seconds)] {
                if let Some(q) = query {
                    ensure!(q.contains("$range"), "tenants.overrides.{tenant}.{key} must use $range");
                }
            }
        }
        Ok(())
    }

    /// The tenant's queries over `range`, overrides applied.
    pub fn slis(&self, tenant: &str, namespace: &str, range: &str) -> Slis {
        let o = self.overrides.get(tenant).cloned().unwrap_or_default();
        let pick = |own: Option<String>, base: &str| fill(own.as_deref().unwrap_or(base), tenant, namespace, range);
        Slis {
            requests: pick(o.requests, &self.slis.requests),
            errors: pick(o.errors, &self.slis.errors),
            p95_seconds: pick(o.p95_seconds, &self.slis.p95_seconds),
        }
    }
}

impl Slis {
    fn entries(&self) -> [(&str, &str); 3] {
        [("requests", &self.requests), ("errors", &self.errors), ("p95Seconds", &self.p95_seconds)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo() -> Tenants {
        #[derive(Deserialize)]
        struct Values {
            tenants: Tenants,
            integrations: Integrations,
        }
        let values: Values = serde_saphyr::from_str(include_str!("../../../charts/blastradius/values.yaml")).unwrap();
        assert!(values.integrations.argocd.is_some());
        values.tenants
    }

    #[test]
    fn checker_chart_defaults_are_valid_and_leave_out_the_demo_fault() {
        #[derive(Deserialize)]
        struct Values {
            tenants: Tenants,
            integrations: Integrations,
            chaos: crate::runs::Config,
        }
        let values: Values =
            serde_saphyr::from_str(include_str!("../../../charts/isolation-checker/values.yaml")).unwrap();
        values.tenants.validate().unwrap();
        assert!(values.integrations.argocd.is_none() && values.integrations.traffic.is_none());
        assert!(!values.chaos.experiments.contains_key("error-burst"));
        let argocd: Integrations = serde_saphyr::from_str("argocd: {namespace: argocd}").unwrap();
        assert_eq!(argocd.argocd.unwrap().app_prefix, "");
        let q = values.tenants.slis("payments", "payments-prod", "60s");
        assert_eq!(q.requests, r#"sum(rate(http_requests_total{namespace="payments-prod"}[60s]))"#);
    }

    #[test]
    fn demo_config_is_valid_and_renders_per_tenant_queries() {
        let t = demo();
        t.validate().unwrap();
        assert_eq!(t.opt_in, OptIn::default());
        let q = t.slis("acme", "tenant-acme", "120s");
        assert_eq!(
            q.errors,
            r#"sum(rate(traefik_service_requests_total{service=~"tenant-acme-workload-.*",code=~"5.."}[120s]))"#
        );
        assert!(!q.p95_seconds.contains('$'));
    }

    #[test]
    fn overrides_replace_only_their_own_queries() {
        let mut t = demo();
        t.overrides.insert(
            "legacy".into(),
            SliOverride { p95_seconds: Some("legacy_p95{window=\"$range\"}".into()), ..SliOverride::default() },
        );
        t.validate().unwrap();
        let q = t.slis("legacy", "legacy-ns", "5m");
        assert_eq!(q.p95_seconds, r#"legacy_p95{window="5m"}"#);
        assert!(q.requests.contains("tenant-legacy-workload"));
    }

    #[test]
    fn validation_names_the_broken_key() {
        let mut t = demo();
        t.slis.requests = "sum(rate(http_requests_total[$range]))".into();
        let e = t.validate().unwrap_err().to_string();
        assert!(e.starts_with("tenants.slis.requests must use $namespace or $tenant"), "{e}");

        let mut t = demo();
        t.overrides.insert("x".into(), SliOverride { errors: Some("up".into()), ..SliOverride::default() });
        assert_eq!(t.validate().unwrap_err().to_string(), "tenants.overrides.x.errors must use $range");

        let e = serde_saphyr::from_str::<Tenants>("selector: a\nslis: {requests: a, errors: b, p95: c}\n");
        assert!(e.is_err());
    }
}
