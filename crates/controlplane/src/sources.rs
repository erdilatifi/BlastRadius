use std::collections::BTreeMap;

use anyhow::{Context, Result};
use k8s_openapi::api::core::v1::Namespace;
use kube::api::{ApiResource, DynamicObject, GroupVersionKind, ListParams};
use kube::{Api, Client};
use serde::Deserialize;

const CHAOS_OPT_IN: &str = "chaos-mesh.org/inject";

pub struct Sources {
    pub http: reqwest::Client,
    prometheus: String,
    pub kube: Client,
}

pub struct TenantRef {
    pub name: String,
    pub tier: String,
}

pub struct AppStatus {
    pub sync: String,
    pub health: String,
}

#[derive(Deserialize)]
struct Response {
    data: Data,
}

#[derive(Deserialize)]
struct Data {
    result: Vec<Sample>,
}

#[derive(Deserialize)]
struct Sample {
    metric: BTreeMap<String, String>,
    value: (f64, String),
}

impl Sources {
    pub async fn new(http: reqwest::Client, prometheus: String) -> Result<Sources> {
        Ok(Sources {
            http,
            prometheus,
            kube: Client::try_default().await?,
        })
    }

    pub async fn tenants(&self) -> Result<Vec<TenantRef>> {
        let namespaces = Api::<Namespace>::all(self.kube.clone())
            .list(&ListParams::default().labels("tenant"))
            .await?;
        Ok(namespaces
            .into_iter()
            .filter_map(|ns| {
                let labels = ns.metadata.labels?;
                Some(TenantRef {
                    name: labels.get("tenant")?.clone(),
                    tier: labels.get("tier").cloned().unwrap_or_default(),
                })
            })
            .collect())
    }

    pub async fn apps(&self) -> Result<BTreeMap<String, AppStatus>> {
        let gvk = GroupVersionKind::gvk("argoproj.io", "v1alpha1", "Application");
        let api: Api<DynamicObject> =
            Api::namespaced_with(self.kube.clone(), "argocd", &ApiResource::from_gvk(&gvk));
        let field = |app: &DynamicObject, path: &str| {
            app.data
                .pointer(path)
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string()
        };
        Ok(api
            .list(&ListParams::default())
            .await?
            .into_iter()
            .filter_map(|app| {
                let tenant = app.metadata.name.as_deref()?.strip_prefix("tenant-")?;
                let status = AppStatus {
                    sync: field(&app, "/status/sync/status"),
                    health: field(&app, "/status/health/status"),
                };
                Some((tenant.to_string(), status))
            })
            .collect())
    }

    pub async fn opted_in(&self, tenant: &str) -> Result<bool> {
        let ns = Api::<Namespace>::all(self.kube.clone())
            .get_opt(&format!("tenant-{tenant}"))
            .await?;
        let meta = ns.map(|n| n.metadata).unwrap_or_default();
        let label = meta.labels.unwrap_or_default().remove("tenant");
        let opt_in = meta.annotations.unwrap_or_default().remove(CHAOS_OPT_IN);
        Ok(label.as_deref() == Some(tenant) && opt_in.as_deref() == Some("enabled"))
    }

    pub async fn by(&self, label: &str, query: &str) -> Result<BTreeMap<String, f64>> {
        self.at(label, query, None).await
    }

    pub async fn at(
        &self,
        label: &str,
        query: &str,
        time: Option<u64>,
    ) -> Result<BTreeMap<String, f64>> {
        let mut params = vec![("query", query.to_string())];
        params.extend(time.map(|t| ("time", t.to_string())));
        let response: Response = self
            .http
            .get(format!("{}/api/v1/query", self.prometheus))
            .query(&params)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .context("prometheus response")?;
        Ok(response
            .data
            .result
            .into_iter()
            .filter_map(|s| {
                let value = s.value.1.parse::<f64>().ok().filter(|v| v.is_finite())?;
                Some((s.metric.get(label)?.clone(), value))
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn tls_clients_build_with_one_crypto_provider() {
        let config = kube::Config::new("https://127.0.0.1:6443".parse().unwrap());
        kube::Client::try_from(config).unwrap();
        reqwest::Client::builder().build().unwrap();
    }
}
