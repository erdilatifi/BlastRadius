use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::State as Extract;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use brcore::{Amount, GATE_PERCENT};
use serde::Serialize;

use crate::sources::{AppStatus, TenantRef};
use crate::{Platform, State};

const AVAILABILITY: &str = r#"1 - tenant:slo_errors:ratio_rate5m{slo="availability"}"#;
const TARGET: &str = r#"1 - tenant:slo_budget:ratio{slo="availability"}"#;
const P95: &str = r#"histogram_quantile(0.95, sum by (le, tenant) (label_replace(rate(traefik_service_request_duration_seconds_bucket{service=~"tenant-.*"}[5m]), "tenant", "$1", "service", "tenant-(.+)-workload-.*")))"#;
const BUDGET: &str = r#"clamp_min(1 - tenant:slo_errors:ratio_rate6h{slo="availability"} / tenant:slo_budget:ratio{slo="availability"}, 0)"#;
const BURN: &str = r#"tenant:slo_errors:ratio_rate1h{slo="availability"} / tenant:slo_budget:ratio{slo="availability"}"#;
const CPU_USED: &str = r#"sum by (namespace) (rate(container_cpu_usage_seconds_total{namespace=~"tenant-.*",container!=""}[5m])) * 1000"#;
const CPU_REQUESTED: &str = r#"sum by (namespace) (kube_pod_container_resource_requests{namespace=~"tenant-.*",resource="cpu"}) * 1000"#;
const MEMORY_USED: &str = r#"sum by (namespace) (container_memory_working_set_bytes{namespace=~"tenant-.*",container!=""}) / 1048576"#;
const MEMORY_REQUESTED: &str = r#"sum by (namespace) (kube_pod_container_resource_requests{namespace=~"tenant-.*",resource="memory"}) / 1048576"#;

type Series = BTreeMap<String, f64>;

pub struct Failure(anyhow::Error);

impl<E: Into<anyhow::Error>> From<E> for Failure {
    fn from(e: E) -> Failure {
        Failure(e.into())
    }
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        (StatusCode::BAD_GATEWAY, format!("{:#}", self.0)).into_response()
    }
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TenantRow {
    name: String,
    tier: String,
    sync: String,
    health: String,
    availability: Option<f64>,
    target: Option<f64>,
    p95_ms: Option<f64>,
    budget_remaining: Option<f64>,
    burn_rate: Option<f64>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    name: String,
    tier: String,
    cpu_used_millis: Option<f64>,
    cpu_requested_millis: Option<f64>,
    memory_used_mib: Option<f64>,
    memory_requested_mib: Option<f64>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CapacityView {
    capacity: Amount,
    committed: Amount,
    gate_percent: u64,
    fits: BTreeMap<String, u64>,
    tenants: Vec<Usage>,
}

pub async fn tenants(Extract(state): Extract<Arc<State>>) -> Result<Json<Vec<TenantRow>>, Failure> {
    let s = &state.sources;
    let (list, apps, availability, target, p95, budget, burn) = tokio::try_join!(
        s.tenants(),
        s.apps(),
        s.by("tenant", AVAILABILITY),
        s.by("tenant", TARGET),
        s.by("tenant", P95),
        s.by("tenant", BUDGET),
        s.by("tenant", BURN),
    )?;
    Ok(Json(tenant_rows(
        list,
        &apps,
        [&availability, &target, &p95, &budget, &burn],
    )))
}

pub async fn capacity(Extract(state): Extract<Arc<State>>) -> Result<Json<CapacityView>, Failure> {
    let s = &state.sources;
    let (list, cpu_used, cpu_requested, memory_used, memory_requested) = tokio::try_join!(
        s.tenants(),
        s.by("namespace", CPU_USED),
        s.by("namespace", CPU_REQUESTED),
        s.by("namespace", MEMORY_USED),
        s.by("namespace", MEMORY_REQUESTED),
    )?;
    let usage = [&cpu_used, &cpu_requested, &memory_used, &memory_requested];
    Ok(Json(capacity_view(&state.platform, list, usage)))
}

fn round(v: f64, places: i32) -> f64 {
    let f = 10f64.powi(places);
    (v * f).round() / f
}

fn tenant_rows(
    list: Vec<TenantRef>,
    apps: &BTreeMap<String, AppStatus>,
    m: [&Series; 5],
) -> Vec<TenantRow> {
    let [availability, target, p95, budget, burn] = m;
    let percent = |s: &Series, name: &str| s.get(name).map(|v| round(v * 100.0, 3));
    let mut rows: Vec<TenantRow> = list
        .into_iter()
        .map(|t| {
            let app = apps.get(&t.name);
            let status =
                |f: fn(&AppStatus) -> &String| app.map_or("Unknown".into(), |a| f(a).clone());
            TenantRow {
                sync: status(|a| &a.sync),
                health: status(|a| &a.health),
                availability: percent(availability, &t.name),
                target: percent(target, &t.name),
                p95_ms: p95.get(&t.name).map(|v| round(v * 1000.0, 0)),
                budget_remaining: percent(budget, &t.name),
                burn_rate: burn.get(&t.name).map(|v| round(*v, 2)),
                name: t.name,
                tier: t.tier,
            }
        })
        .collect();
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rows
}

fn capacity_view(platform: &Platform, list: Vec<TenantRef>, m: [&Series; 4]) -> CapacityView {
    let [cpu_used, cpu_requested, memory_used, memory_requested] = m;
    let committed = list
        .iter()
        .filter_map(|t| platform.tiers.get(&t.tier))
        .fold(Amount::default(), |sum, tier| sum.plus(tier.quota));
    let fits = platform
        .tiers
        .iter()
        .map(|(name, tier)| (name.clone(), platform.capacity.fits(committed, tier.quota)))
        .collect();
    let mut tenants: Vec<Usage> = list
        .into_iter()
        .map(|t| {
            let ns = format!("tenant-{}", t.name);
            let get = |s: &Series| s.get(&ns).map(|v| round(*v, 0));
            Usage {
                cpu_used_millis: get(cpu_used),
                cpu_requested_millis: get(cpu_requested),
                memory_used_mib: get(memory_used),
                memory_requested_mib: get(memory_requested),
                name: t.name,
                tier: t.tier,
            }
        })
        .collect();
    tenants.sort_by(|a, b| a.name.cmp(&b.name));
    CapacityView {
        capacity: platform.capacity,
        committed,
        gate_percent: GATE_PERCENT,
        fits,
        tenants,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tenant(name: &str, tier: &str) -> TenantRef {
        TenantRef {
            name: name.into(),
            tier: tier.into(),
        }
    }

    fn series(pairs: &[(&str, f64)]) -> Series {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn tenant_rows_are_display_ready_and_tolerate_missing_data() {
        let apps = BTreeMap::from([(
            "acme".to_string(),
            AppStatus {
                sync: "Synced".into(),
                health: "Healthy".into(),
            },
        )]);
        let rows = tenant_rows(
            vec![tenant("globex", "medium"), tenant("acme", "small")],
            &apps,
            [
                &series(&[("acme", 0.99512)]),
                &series(&[("acme", 0.995)]),
                &series(&[("acme", 0.0423)]),
                &series(&[("acme", 0.4)]),
                &series(&[("acme", 14.444)]),
            ],
        );
        assert_eq!(rows[0].name, "acme");
        assert_eq!(rows[0].availability, Some(99.512));
        assert_eq!(rows[0].p95_ms, Some(42.0));
        assert_eq!(rows[0].budget_remaining, Some(40.0));
        assert_eq!(rows[0].burn_rate, Some(14.44));
        assert_eq!(rows[1].health, "Unknown");
        assert_eq!(rows[1].availability, None);
    }

    #[test]
    fn capacity_counts_committed_quota_and_what_still_fits() {
        let platform: Platform = serde_saphyr::from_str(
            "capacity: {cpuMillis: 4000, memoryMiB: 4096, pods: 60}\n\
             tiers:\n  small:\n    requests: {cpuMillis: 50, memoryMiB: 32}\n    limits: {cpuMillis: 200, memoryMiB: 64}\n    quota: {cpuMillis: 400, memoryMiB: 256, pods: 6}\n",
        )
        .unwrap();
        let view = capacity_view(
            &platform,
            vec![tenant("acme", "small"), tenant("globex", "small")],
            [
                &series(&[("tenant-acme", 12.4)]),
                &Series::new(),
                &Series::new(),
                &Series::new(),
            ],
        );
        assert_eq!(view.committed.cpu_millis, 800);
        assert_eq!(view.fits["small"], 6);
        assert_eq!(view.tenants[0].cpu_used_millis, Some(12.0));
    }
}
