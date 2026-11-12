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
use crate::score::{self, Score, Thresholds, Window};

const POLL: Duration = Duration::from_secs(5);
const SELECTOR: &str = "app.kubernetes.io/name=workload";
const TRAFFIC: &str = r#"traefik_service_requests_total{service=~"tenant-.*"FILTER}[RANGE]"#;
const LATENCY: &str = r#"histogram_quantile(0.95, sum by (le, tenant) (label_replace(rate(traefik_service_request_duration_seconds_bucket{service=~"tenant-.*"}[RANGE]), "tenant", "$1", "service", "tenant-(.+)-workload-.*"))) * 1000"#;
const RESTARTS: &str =
    r#"sum by (namespace) (increase(kube_pod_container_status_restarts_total{namespace=~"tenant-.*"}[RANGE]))"#;
const UNREADY: &str =
    r#"max_over_time(sum by (namespace) (kube_pod_status_ready{namespace=~"tenant-.*",condition="false"})[RANGE:15s])"#;

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
    if !target.chars().all(valid) || !state.sources.opted_in(target).await.map_err(lookup)? {
        return Err(refuse(StatusCode::FORBIDDEN, "only tenant namespaces that opted in to chaos can be targeted"));
    }
    let started_at = now();
    let run = Run {
        id: format!("{started_at}-{experiment}-{target}"),
        commit: state.runs.commit.clone(),
        experiment: experiment.into(),
        target: target.into(),
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
    tokio::time::sleep(Duration::from_secs(config.baseline_seconds)).await;

    let injected_at = now();
    let baseline = windows(state, injected_at, injected_at - run.started_at).await?;
    let handle = inject(state, id, &run.target, plan).await?;
    state.runs.update(id, |r| {
        r.phase = Phase::Inject;
        r.injected_at = Some(injected_at);
    });
    watch(state, id, injected_at, plan.inject_seconds, |_| async { false }).await;
    let removal = remove(state, id, handle).await;

    let removed_at = now();
    state.runs.update(id, |r| {
        r.phase = Phase::Recovery;
        r.removed_at = Some(removed_at);
    });
    removal?;
    let before = baseline.get(&run.target).copied().unwrap_or_default();
    let recovered = watch(state, id, injected_at, config.recovery_seconds, |at| {
        let (target, thresholds) = (run.target.clone(), config.thresholds);
        async move {
            match windows(state, at, 60).await {
                Ok(w) => w
                    .get(&target)
                    .is_some_and(|w| thresholds.moved(&before, &Window { restarts: 0.0, ..*w }).is_empty()),
                Err(_) => false,
            }
        }
    })
    .await;

    state.runs.update(id, |r| {
        r.phase = Phase::Scoring;
        r.recover_seconds = recovered.map(|at| at - removed_at);
    });
    let during = windows(state, removed_at, removed_at - injected_at).await?;
    let tenants: Vec<String> = state.sources.tenants().await?.into_iter().map(|t| t.name).collect();
    let score = score::score(&config.thresholds, &run.target, &tenants, &baseline, &during);
    state.runs.update(id, |r| r.score = Some(score));
    Ok(())
}

async fn watch<F, Fut>(state: &State, id: &str, injected_at: u64, seconds: u64, done: F) -> Option<u64>
where
    F: Fn(u64) -> Fut,
    Fut: Future<Output = bool>,
{
    let target = state.runs.get(id)?.target;
    let alert = format!(r#"ALERTS{{alertstate="firing",alertname=~"Tenant.+Burn",tenant="{target}"}}"#);
    let deadline = now() + seconds;
    while now() < deadline {
        tokio::time::sleep(POLL).await;
        let at = now();
        if state.sources.by("tenant", &alert).await.is_ok_and(|a| !a.is_empty()) {
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

async fn windows(state: &State, at: u64, seconds: u64) -> Result<BTreeMap<String, Window>> {
    let range = format!("{seconds}s");
    let traffic = |filter: &str| {
        let selector = TRAFFIC.replace("FILTER", filter).replace("RANGE", &range);
        format!(
            r#"sum by (tenant) (label_replace(rate({selector}), "tenant", "$1", "service", "tenant-(.+)-workload-.*"))"#
        )
    };
    let errors = format!("100 * ({} / {all} or 0 * {all})", traffic(r#",code=~"5..""#), all = traffic(""));
    let [latency, restarts, unready] = [LATENCY, RESTARTS, UNREADY].map(|q| q.replace("RANGE", &range));
    let s = &state.sources;
    let (errors, p95, restarts, unready) = tokio::try_join!(
        s.at("tenant", &errors, Some(at)),
        s.at("tenant", &latency, Some(at)),
        s.at("namespace", &restarts, Some(at)),
        s.at("namespace", &unready, Some(at)),
    )?;
    Ok(errors
        .into_iter()
        .map(|(tenant, error_pp)| {
            let ns = format!("tenant-{tenant}");
            let window = Window {
                error_pp,
                p95_ms: p95.get(&tenant).copied().unwrap_or_default(),
                restarts: restarts.get(&ns).copied().unwrap_or_default().round(),
                unready: unready.get(&ns).copied().unwrap_or_default(),
            };
            (tenant, window)
        })
        .collect())
}

async fn inject(state: &State, id: &str, target: &str, plan: &Experiment) -> Result<Handle> {
    let ns = format!("tenant-{target}");
    let kube = state.sources.kube.clone();
    if let Some(chaos) = &plan.chaos {
        let gvk = GroupVersionKind::gvk("chaos-mesh.org", "v1alpha1", &chaos.kind);
        let resource = ApiResource::from_gvk_with_plural(&gvk, &chaos.kind.to_lowercase());
        let api = Api::<DynamicObject>::namespaced_with(kube, &ns, &resource);
        let mut spec = chaos.spec.clone();
        spec["selector"] = json!({"namespaces": [ns], "labelSelectors": {"app.kubernetes.io/name": "workload"}});
        spec["duration"] = json!(format!("{}s", plan.inject_seconds));
        let mut object = DynamicObject::new(id, &resource).within(&ns);
        object.data = json!({ "spec": spec });
        api.create(&PostParams::default(), &object).await?;
        return Ok(Handle::Chaos(Box::new(api)));
    }
    let pods = Api::<Pod>::namespaced(kube, &ns).list(&ListParams::default().labels(SELECTOR)).await?;
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
