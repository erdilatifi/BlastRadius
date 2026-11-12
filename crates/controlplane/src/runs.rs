use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use axum::http::StatusCode;
use k8s_openapi::api::core::v1::Pod;
use kube::Api;
use kube::api::{ApiResource, DeleteParams, DynamicObject, GroupVersionKind, ListParams, PostParams};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::State;
use crate::config::fill;
use crate::score::{self, Score, Thresholds, Window};
use crate::sources::{self, Sli, TenantRef};

const POLL: Duration = Duration::from_secs(5);
const RESTARTS: &str =
    r#"sum by (namespace) (increase(kube_pod_container_status_restarts_total{namespace=~"NAMESPACES"}[RANGE]))"#;
const UNREADY: &str = r#"max_over_time(sum by (namespace) (kube_pod_status_ready{namespace=~"NAMESPACES",condition="false"})[RANGE:15s])"#;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chaos {
    kind: String,
    spec: Value,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Experiment {
    pub hypothesis: String,
    pub inject_seconds: u64,
    #[serde(skip_serializing)]
    chaos: Option<Chaos>,
    #[serde(skip_serializing)]
    fault: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    baseline_seconds: u64,
    recovery_seconds: u64,
    thresholds: Thresholds,
    pub experiments: BTreeMap<String, Experiment>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    #[default]
    Baseline,
    Inject,
    Recovery,
    Scoring,
    Done,
    Failed,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    id: String,
    commit: String,
    pub experiment: String,
    pub target: String,
    pub namespace: String,
    phase: Phase,
    started_at: u64,
    pub injected_at: Option<u64>,
    pub removed_at: Option<u64>,
    finished_at: Option<u64>,
    detect_seconds: Option<u64>,
    recover_seconds: Option<u64>,
    thresholds: Thresholds,
    score: Option<Score>,
    error: Option<String>,
}

pub struct Runs {
    commit: String,
    list: Mutex<Vec<Run>>,
}

enum Handle {
    Chaos(Box<Api<DynamicObject>>),
    Fault(Vec<String>),
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Runs {
    pub fn new(commit: String) -> Runs {
        Runs { commit, list: Mutex::new(Vec::new()) }
    }

    pub fn all(&self) -> Vec<Run> {
        self.list.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn update(&self, id: &str, f: impl FnOnce(&mut Run)) {
        let mut list = self.list.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(run) = list.iter_mut().find(|r| r.id == id) {
            f(run);
        }
    }

    pub fn get(&self, id: &str) -> Option<Run> {
        self.all().into_iter().find(|r| r.id == id)
    }
}

pub async fn start(state: Arc<State>, experiment: &str, target: &str) -> Result<Run, (StatusCode, String)> {
    let config = &state.platform.chaos;
    let refuse = |code, why: &str| (code, why.to_string());
    let plan = config.experiments.get(experiment).cloned();
    let plan = plan.ok_or(refuse(StatusCode::NOT_FOUND, "no such experiment in the catalog"))?;
    let valid = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
    let lookup = |e: anyhow::Error| (StatusCode::BAD_GATEWAY, format!("{e:#}"));
    let tenant = match target.chars().all(valid) {
        true => state.sources.tenant(target).await.map_err(lookup)?,
        false => None,
    };
    let Some(tenant) = tenant.filter(|t| t.opted_in) else {
        return Err(refuse(StatusCode::FORBIDDEN, "only tenant namespaces that opted in to chaos can be targeted"));
    };
    let started_at = now();
    let run = Run {
        id: format!("{started_at}-{experiment}-{target}"),
        commit: state.runs.commit.clone(),
        experiment: experiment.into(),
        target: tenant.name,
        namespace: tenant.namespace,
        started_at,
        thresholds: config.thresholds,
        ..Run::default()
    };
    {
        let mut list = state.runs.list.lock().unwrap_or_else(|e| e.into_inner());
        if list.iter().any(|r| !matches!(r.phase, Phase::Done | Phase::Failed)) {
            return Err(refuse(StatusCode::CONFLICT, "another run is in progress"));
        }
        list.push(run.clone());
    }
    let id = run.id.clone();
    tokio::spawn(async move {
        let outcome = timeline(&state, &id, &plan).await;
        state.runs.update(&id, |r| {
            r.finished_at = Some(now());
            match outcome {
                Ok(()) => r.phase = Phase::Done,
                Err(e) => {
                    r.phase = Phase::Failed;
                    r.error = Some(format!("{e:#}"));
                }
            }
        });
        tracing::info!(run = id, "run finished");
    });
    Ok(run)
}

async fn timeline(state: &State, id: &str, plan: &Experiment) -> Result<()> {
    let config = &state.platform.chaos;
    let run = state.runs.get(id).context("run vanished")?;
    // The bystanders are fixed when the run starts, so every window measures the same tenants.
    let tenants = state.sources.tenants().await?;
    let target = tenants.iter().find(|t| t.name == run.target).cloned().context("target tenant vanished")?;
    tokio::time::sleep(Duration::from_secs(config.baseline_seconds)).await;

    let injected_at = now();
    let baseline = windows(state, &tenants, injected_at, injected_at - run.started_at).await?;
    let handle = inject(state, id, &run.namespace, plan).await?;
    state.runs.update(id, |r| {
        r.phase = Phase::Inject;
        r.injected_at = Some(injected_at);
    });
    watch(state, &target, id, injected_at, plan.inject_seconds, |_| async { false }).await;
    let removal = remove(state, id, handle).await;

    let removed_at = now();
    state.runs.update(id, |r| {
        r.phase = Phase::Recovery;
        r.removed_at = Some(removed_at);
    });
    removal?;
    let before = baseline.get(&run.target).copied().unwrap_or_default();
    let (only_target, name) = (std::slice::from_ref(&target), target.name.as_str());
    let recovered = watch(state, &target, id, injected_at, config.recovery_seconds, |at| {
        let thresholds = config.thresholds;
        async move {
            match windows(state, only_target, at, 60).await {
                Ok(w) => {
                    w.get(name).is_some_and(|w| thresholds.moved(&before, &Window { restarts: 0.0, ..*w }).is_empty())
                }
                Err(_) => false,
            }
        }
    })
    .await;

    state.runs.update(id, |r| {
        r.phase = Phase::Scoring;
        r.recover_seconds = recovered.map(|at| at - removed_at);
    });
    let during = windows(state, &tenants, removed_at, removed_at - injected_at).await?;
    let names: Vec<String> = tenants.into_iter().map(|t| t.name).collect();
    let score = score::score(&config.thresholds, &run.target, &names, &baseline, &during);
    state.runs.update(id, |r| r.score = Some(score));
    Ok(())
}

/// Polls until `done` or the deadline, recording time to detect when the configured alert fires.
async fn watch<F, Fut>(
    state: &State,
    target: &TenantRef,
    id: &str,
    injected_at: u64,
    seconds: u64,
    done: F,
) -> Option<u64>
where
    F: Fn(u64) -> Fut,
    Fut: Future<Output = bool>,
{
    let alert = state.platform.integrations.detect_alert.as_deref();
    let alert = alert.map(|q| fill(q, &target.name, &target.namespace, ""));
    let deadline = now() + seconds;
    while now() < deadline {
        tokio::time::sleep(POLL).await;
        let at = now();
        if let Some(alert) = &alert
            && state.sources.any(alert).await.unwrap_or(false)
        {
            state.runs.update(id, |r| {
                r.detect_seconds.get_or_insert(at - injected_at);
            });
        }
        if done(at).await {
            return Some(at);
        }
    }
    None
}

/// Each tenant's window over the `seconds` before `at`: its configured SLIs plus
/// restarts and unready pods from kube-state-metrics.
async fn windows(state: &State, tenants: &[TenantRef], at: u64, seconds: u64) -> Result<BTreeMap<String, Window>> {
    if tenants.is_empty() {
        return Ok(BTreeMap::new());
    }
    let range = format!("{seconds}s");
    let namespaces = sources::namespaces(tenants);
    let [restarts, unready] =
        [RESTARTS, UNREADY].map(|q| q.replace("NAMESPACES", &namespaces).replace("RANGE", &range));
    let s = &state.sources;
    let (slis, restarts, unready) = tokio::try_join!(
        s.slis(tenants, &range, Some(at)),
        s.at("namespace", &restarts, Some(at)),
        s.at("namespace", &unready, Some(at)),
    )?;
    Ok(assemble(tenants, &slis, &restarts, &unready))
}

/// A tenant gets a window only if it had traffic; silence is what scoring calls "no traffic".
fn assemble(
    tenants: &[TenantRef],
    slis: &BTreeMap<String, Sli>,
    restarts: &BTreeMap<String, f64>,
    unready: &BTreeMap<String, f64>,
) -> BTreeMap<String, Window> {
    tenants
        .iter()
        .filter_map(|t| {
            let sli = slis.get(&t.name)?;
            let window = Window {
                error_pp: sli.error_ratio? * 100.0,
                p95_ms: sli.p95_seconds.map_or(0.0, |s| s * 1000.0),
                restarts: restarts.get(&t.namespace).copied().unwrap_or_default().round(),
                unready: unready.get(&t.namespace).copied().unwrap_or_default(),
            };
            Some((t.name.clone(), window))
        })
        .collect()
}

/// Chaos Mesh selector for the target's pods: the namespace, narrowed by `targetPods` when set.
fn selector(ns: &str, pods: &BTreeMap<String, String>) -> Value {
    let mut selector = json!({ "namespaces": [ns] });
    if !pods.is_empty() {
        selector["labelSelectors"] = json!(pods);
    }
    selector
}

async fn inject(state: &State, id: &str, ns: &str, plan: &Experiment) -> Result<Handle> {
    let kube = state.sources.kube.clone();
    let pods = &state.platform.tenants.target_pods;
    if let Some(chaos) = &plan.chaos {
        let gvk = GroupVersionKind::gvk("chaos-mesh.org", "v1alpha1", &chaos.kind);
        let resource = ApiResource::from_gvk_with_plural(&gvk, &chaos.kind.to_lowercase());
        let api = Api::<DynamicObject>::namespaced_with(kube, ns, &resource);
        let mut spec = chaos.spec.clone();
        spec["selector"] = selector(ns, pods);
        spec["duration"] = json!(format!("{}s", plan.inject_seconds));
        let mut object = DynamicObject::new(id, &resource).within(ns);
        object.data = json!({ "spec": spec });
        api.create(&PostParams::default(), &object).await?;
        return Ok(Handle::Chaos(Box::new(api)));
    }
    let labels: Vec<String> = pods.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let pods = Api::<Pod>::namespaced(kube, ns).list(&ListParams::default().labels(&labels.join(","))).await?;
    let urls: Vec<String> =
        pods.into_iter().filter_map(|p| p.status?.pod_ip).map(|ip| format!("http://{ip}:9090/admin/fault")).collect();
    set_fault(state, &urls, plan.fault.as_ref().unwrap_or(&json!({}))).await?;
    Ok(Handle::Fault(urls))
}

async fn remove(state: &State, id: &str, handle: Handle) -> Result<()> {
    match handle {
        Handle::Chaos(api) => {
            api.delete(id, &DeleteParams::default()).await?;
        }
        Handle::Fault(urls) => set_fault(state, &urls, &json!({})).await?,
    }
    Ok(())
}

async fn set_fault(state: &State, urls: &[String], fault: &Value) -> Result<()> {
    for url in urls {
        state.sources.http.put(url).json(fault).send().await?.error_for_status()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faults_select_the_namespace_narrowed_by_target_pods() {
        assert_eq!(selector("payments", &BTreeMap::new()), json!({"namespaces": ["payments"]}));
        let pods = BTreeMap::from([("app".to_string(), "api".to_string())]);
        assert_eq!(selector("payments", &pods), json!({"namespaces": ["payments"], "labelSelectors": {"app": "api"}}));
    }

    fn tenant(name: &str) -> TenantRef {
        TenantRef { name: name.into(), namespace: format!("{name}-prod"), tier: String::new(), opted_in: true }
    }

    #[test]
    fn windows_need_traffic_and_take_pods_from_the_tenant_namespace() {
        let tenants = ["acme", "globex", "initech", "umbrella"].map(tenant);
        let slis = BTreeMap::from([
            ("acme".to_string(), Sli { error_ratio: Some(0.012), p95_seconds: Some(0.0425) }),
            ("globex".to_string(), Sli { error_ratio: Some(0.0), p95_seconds: None }),
            ("initech".to_string(), Sli { error_ratio: None, p95_seconds: Some(0.01) }),
        ]);
        let restarts = BTreeMap::from([("acme-prod".to_string(), 1.9), ("acme".to_string(), 7.0)]);
        let unready = BTreeMap::from([("globex-prod".to_string(), 1.0)]);
        let w = assemble(&tenants, &slis, &restarts, &unready);
        assert_eq!(w.keys().collect::<Vec<_>>(), ["acme", "globex"]);
        assert_eq!(w["acme"], Window { error_pp: 1.2, p95_ms: 42.5, restarts: 2.0, unready: 0.0 });
        assert_eq!(w["globex"], Window { error_pp: 0.0, p95_ms: 0.0, restarts: 0.0, unready: 1.0 });
    }
}
