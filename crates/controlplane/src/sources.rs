use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use k8s_openapi::api::core::v1::Namespace;
use kube::api::{ApiResource, DynamicObject, GroupVersionKind, ListParams};
use kube::{Api, Client};
use serde_json::Value;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::config::{ArgoCd, Tenants};

/// Prometheus queries in flight at once when every tenant gets its own query.
const PARALLEL_QUERIES: usize = 16;

pub struct Sources {
    pub http: reqwest::Client,
    prometheus: Prometheus,
    pub kube: Client,
    config: Tenants,
    argocd: Option<ArgoCd>,
}

#[derive(Clone)]
struct Prometheus {
    http: reqwest::Client,
    url: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TenantRef {
    pub name: String,
    pub namespace: String,
    pub tier: String,
    pub opted_in: bool,
}

pub struct AppStatus {
    pub sync: String,
    pub health: String,
}

/// One tenant's service level indicators over a window, `None` where Prometheus had no data.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sli {
    pub error_ratio: Option<f64>,
    pub p95_seconds: Option<f64>,
}

fn dns_label(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 63
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

/// Turns the namespaces the selector matched into tenants, sorted by name.
/// Namespaces without a usable name, and later duplicates of a name, are skipped.
pub fn tenant_refs(config: &Tenants, namespaces: Vec<Namespace>) -> Vec<TenantRef> {
    let mut tenants: BTreeMap<String, TenantRef> = BTreeMap::new();
    for ns in namespaces {
        let Some(namespace) = ns.metadata.name else { continue };
        let labels = ns.metadata.labels.unwrap_or_default();
        let annotations = ns.metadata.annotations.unwrap_or_default();
        let name = match &config.name_label {
            Some(label) => labels.get(label).cloned().unwrap_or_default(),
            None => namespace.clone(),
        };
        if !dns_label(&name) {
            tracing::warn!(namespace, name, "skipping namespace: tenant name is not a DNS label");
            continue;
        }
        if let Some(first) = tenants.get(&name) {
            tracing::warn!(namespace, name, first = first.namespace, "skipping namespace: tenant name already taken");
            continue;
        }
        let tier = config.tier_label.as_ref().and_then(|l| labels.get(l)).cloned().unwrap_or_default();
        let opted_in = annotations.get(&config.opt_in.annotation) == Some(&config.opt_in.value);
        tenants.insert(name.clone(), TenantRef { name, namespace, tier, opted_in });
    }
    tenants.into_values().collect()
}

/// Error ratio from request and error rates: no requests means no ratio, no error series means none failed.
pub fn error_ratio(requests: Option<f64>, errors: Option<f64>) -> Option<f64> {
    requests.filter(|r| *r > 0.0).map(|r| errors.unwrap_or_default() / r)
}

/// PromQL alternation that matches exactly these namespaces.
pub fn namespaces(list: &[TenantRef]) -> String {
    list.iter().map(|t| t.namespace.as_str()).collect::<Vec<_>>().join("|")
}

impl Prometheus {
    async fn query(&self, query: &str, time: Option<u64>) -> Result<Vec<Value>> {
        let mut params = vec![("query", query.to_string())];
        params.extend(time.map(|t| ("time", t.to_string())));
        let body: Value = self
            .http
            .get(format!("{}/api/v1/query", self.url))
            .query(&params)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .context("prometheus response")?;
        Ok(body["data"]["result"].as_array().cloned().unwrap_or_default())
    }

    async fn scalar(&self, query: &str, time: Option<u64>) -> Result<Option<f64>> {
        let result = self.query(query, time).await?;
        if result.len() > 1 {
            bail!("{query} returned {} series, expected one; aggregate it, for example with sum(...)", result.len());
        }
        Ok(result.first().and_then(value))
    }
}

fn value(series: &Value) -> Option<f64> {
    series["value"][1].as_str()?.parse::<f64>().ok().filter(|v| v.is_finite())
}

impl Sources {
    pub async fn new(
        http: reqwest::Client,
        prometheus: String,
        config: Tenants,
        argocd: Option<ArgoCd>,
    ) -> Result<Sources> {
        let prometheus = Prometheus { http: http.clone(), url: prometheus };
        Ok(Sources { http, prometheus, kube: Client::try_default().await?, config, argocd })
    }

    pub async fn tenants(&self) -> Result<Vec<TenantRef>> {
        let params = ListParams::default().labels(&self.config.selector);
        let namespaces = Api::<Namespace>::all(self.kube.clone()).list(&params).await?;
        Ok(tenant_refs(&self.config, namespaces.items))
    }

    pub async fn tenant(&self, name: &str) -> Result<Option<TenantRef>> {
        Ok(self.tenants().await?.into_iter().find(|t| t.name == name))
    }

    pub async fn apps(&self) -> Result<BTreeMap<String, AppStatus>> {
        let Some(argocd) = &self.argocd else {
            return Ok(BTreeMap::new());
        };
        let gvk = GroupVersionKind::gvk("argoproj.io", "v1alpha1", "Application");
        let api: Api<DynamicObject> =
            Api::namespaced_with(self.kube.clone(), &argocd.namespace, &ApiResource::from_gvk(&gvk));
        let field = |app: &DynamicObject, path: &str| {
            app.data.pointer(path).and_then(|v| v.as_str()).unwrap_or("Unknown").to_string()
        };
        Ok(api
            .list(&ListParams::default())
            .await?
            .into_iter()
            .filter_map(|app| {
                let tenant = app.metadata.name.as_deref()?.strip_prefix(argocd.app_prefix.as_str())?;
                let status = AppStatus {
                    sync: field(&app, "/status/sync/status"),
                    health: field(&app, "/status/health/status"),
                };
                Some((tenant.to_string(), status))
            })
            .collect())
    }

    pub fn argocd(&self) -> bool {
        self.argocd.is_some()
    }

    /// Every tenant's configured SLIs over the `range` before `time`.
    pub async fn slis(&self, list: &[TenantRef], range: &str, time: Option<u64>) -> Result<BTreeMap<String, Sli>> {
        let limit = Arc::new(Semaphore::new(PARALLEL_QUERIES));
        let mut set = JoinSet::new();
        for t in list {
            let q = self.config.slis(&t.name, &t.namespace, range);
            for (kind, query) in [(0, q.requests), (1, q.errors), (2, q.p95_seconds)] {
                let (prometheus, limit, name) = (self.prometheus.clone(), limit.clone(), t.name.clone());
                set.spawn(async move {
                    let _permit = limit.acquire_owned().await?;
                    let value = prometheus.scalar(&query, time).await.with_context(|| format!("{name} SLI"))?;
                    anyhow::Ok((name, kind, value))
                });
            }
        }
        let mut raw: BTreeMap<String, [Option<f64>; 3]> = BTreeMap::new();
        while let Some(done) = set.join_next().await {
            let (name, kind, value) = done??;
            raw.entry(name).or_default()[kind] = value;
        }
        Ok(raw
            .into_iter()
            .map(|(name, [requests, errors, p95_seconds])| {
                (name, Sli { error_ratio: error_ratio(requests, errors), p95_seconds })
            })
            .collect())
    }

    /// Whether the query returns any series right now.
    pub async fn any(&self, query: &str) -> Result<bool> {
        Ok(!self.prometheus.query(query, None).await?.is_empty())
    }

    pub async fn by(&self, label: &str, query: &str) -> Result<BTreeMap<String, f64>> {
        self.at(label, query, None).await
    }

    pub async fn at(&self, label: &str, query: &str, time: Option<u64>) -> Result<BTreeMap<String, f64>> {
        Ok(self
            .prometheus
            .query(query, time)
            .await?
            .iter()
            .filter_map(|s| Some((s["metric"][label].as_str()?.to_string(), value(s)?)))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    use super::*;
    use crate::config::{OptIn, Slis};

    #[tokio::test]
    async fn tls_clients_build_with_one_crypto_provider() {
        let config = kube::Config::new("https://127.0.0.1:6443".parse().unwrap());
        kube::Client::try_from(config).unwrap();
        reqwest::Client::builder().build().unwrap();
    }

    fn config(name_label: Option<&str>) -> Tenants {
        Tenants {
            selector: "team".into(),
            name_label: name_label.map(Into::into),
            tier_label: Some("tier".into()),
            opt_in: OptIn::default(),
            target_pods: BTreeMap::new(),
            slis: Slis { requests: String::new(), errors: String::new(), p95_seconds: String::new() },
            overrides: BTreeMap::new(),
        }
    }

    fn ns(name: &str, labels: &[(&str, &str)], annotations: &[(&str, &str)]) -> Namespace {
        let map = |pairs: &[(&str, &str)]| Some(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect());
        Namespace {
            metadata: ObjectMeta {
                name: Some(name.into()),
                labels: map(labels),
                annotations: map(annotations),
                ..ObjectMeta::default()
            },
            ..Namespace::default()
        }
    }

    #[test]
    fn tenants_are_named_by_label_or_namespace_and_opt_in_by_annotation() {
        let list = vec![
            ns("payments-prod", &[("team", "payments"), ("tier", "large")], &[("chaos-mesh.org/inject", "enabled")]),
            ns("search", &[("team", "search")], &[("chaos-mesh.org/inject", "disabled")]),
        ];
        let by_label = tenant_refs(&config(Some("team")), list.clone());
        assert_eq!(
            by_label[0],
            TenantRef {
                name: "payments".into(),
                namespace: "payments-prod".into(),
                tier: "large".into(),
                opted_in: true
            }
        );
        assert!(!by_label[1].opted_in);
        let by_namespace = tenant_refs(&config(None), list);
        assert_eq!(by_namespace[0].name, "payments-prod");
        assert_eq!(by_namespace[1].tier, "");
    }

    #[test]
    fn unusable_names_and_duplicates_are_skipped() {
        let list = vec![
            ns("a", &[("team", "Billing")], &[]),
            ns("b", &[], &[]),
            ns("c", &[("team", "ops")], &[]),
            ns("d", &[("team", "ops")], &[]),
        ];
        let refs = tenant_refs(&config(Some("team")), list);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].namespace, "c");
    }

    #[test]
    fn error_ratio_needs_traffic_but_not_error_series() {
        assert_eq!(error_ratio(None, Some(1.0)), None);
        assert_eq!(error_ratio(Some(0.0), None), None);
        assert_eq!(error_ratio(Some(4.0), None), Some(0.0));
        assert_eq!(error_ratio(Some(4.0), Some(1.0)), Some(0.25));
    }
}
