use std::collections::BTreeMap;

use anyhow::{Context, Result};
use k8s_openapi::api::core::v1::Namespace;
use kube::api::{ApiResource, DynamicObject, GroupVersionKind, ListParams};
use kube::{Api, Client};
use serde::Deserialize;

pub struct Sources {
    http: reqwest::Client,
    prometheus: String,
    kube: Client,
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

    pub async fn by(&self, label: &str, query: &str) -> Result<BTreeMap<String, f64>> {
        let response: Response = self
            .http
            .get(format!("{}/api/v1/query", self.prometheus))
            .query(&[("query", query)])
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
