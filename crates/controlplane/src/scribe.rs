use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use axum::Json;
use axum::extract::State as Extract;
use axum::http::StatusCode;
use brcore::GitHub;
use k8s_openapi::api::core::v1::Event;
use k8s_openapi::jiff::Timestamp;
use kube::Api;
use kube::api::ListParams;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::State;
use crate::api::{BUDGET, BURN, P95};
use crate::postmortem::{self, Facts, Leads};

const OOM: &str = r#"sum by (namespace) (kube_pod_container_status_last_terminated_reason{reason="OOMKilled"})"#;
const SIGNALS: [(&str, &str, &str, &str); 7] = [
    ("Error ratio, 5m", "tenant", r#"100 * tenant:slo_errors:ratio_rate5m{slo="availability"}"#, "%"),
    ("Burn rate, 1h", "tenant", BURN, "x"),
    ("Error budget left", "tenant", BUDGET, ""),
    ("p95 latency", "tenant", P95, " s"),
    (
        "Restarts, 30m",
        "namespace",
        r#"sum by (namespace) (increase(kube_pod_container_status_restarts_total[30m]))"#,
        "",
    ),
    (
        "CPU throttled, 5m",
        "namespace",
        r#"100 * sum by (namespace) (rate(container_cpu_cfs_throttled_periods_total[5m])) / sum by (namespace) (rate(container_cpu_cfs_periods_total[5m]))"#,
        "%",
    ),
    (
        "Memory of limit",
        "namespace",
        r#"100 * sum by (namespace) (container_memory_working_set_bytes{container!=""}) / sum by (namespace) (kube_pod_container_resource_limits{resource="memory"})"#,
        "%",
    ),
];

pub struct Scribe {
    pub loki: String,
    pub api: String,
    pub repo: String,
    pub token: Option<String>,
    pub incidents: Mutex<BTreeMap<String, Incident>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Incident {
    fingerprint: String,
    #[serde(flatten)]
    facts: Facts,
    detect_seconds: Option<i64>,
    markdown: String,
    pr: Option<String>,
}

#[derive(Deserialize)]
pub struct Hook {
    alerts: Vec<Alert>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Alert {
    status: String,
    labels: BTreeMap<String, String>,
    annotations: BTreeMap<String, String>,
    starts_at: Timestamp,
    ends_at: Timestamp,
    fingerprint: String,
}

pub async fn incidents(Extract(state): Extract<Arc<State>>) -> Json<Vec<Incident>> {
    let all = state.scribe.incidents.lock().unwrap_or_else(|e| e.into_inner());
    Json(all.values().cloned().collect())
}

pub async fn hook(Extract(state): Extract<Arc<State>>, Json(hook): Json<Hook>) -> StatusCode {
    for alert in hook.alerts {
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(&state, alert).await {
                tracing::error!(error = format!("{e:#}"), "scribe");
            }
        });
    }
    StatusCode::OK
}

async fn handle(state: &State, alert: Alert) -> Result<()> {
    let Some(tenant) = alert.labels.get("tenant") else {
        return Ok(());
    };
    let known = state.scribe.incidents.lock().unwrap_or_else(|e| e.into_inner()).get(&alert.fingerprint).cloned();
    let (mut facts, detect_seconds) = match (alert.status.as_str(), known) {
        ("firing", None) => collect(state, &alert, tenant).await?,
        ("resolved", Some(i)) if i.facts.resolved_at.is_none() => (i.facts, i.detect_seconds),
        _ => return Ok(()),
    };
    if alert.status == "resolved" {
        let at = alert.ends_at.as_second();
        facts.resolved_at = Some(at);
        facts.timeline.push((at, "alertmanager".into(), format!("{} resolved", facts.alert)));
    }
    let markdown = postmortem::render(&facts);
    let pr = match &state.scribe.token {
        Some(token) => {
            let gh = GitHub::new(&state.scribe.api, &state.scribe.repo, token)?;
            let day = postmortem::time(facts.fired_at)[..10].to_string();
            let path = format!("incidents/{day}-{tenant}-{}.md", alert.fingerprint);
            let title = format!("Post-mortem draft: {} on {tenant}", facts.alert);
            let branch = format!("postmortem-{}", alert.fingerprint);
            Some(gh.upsert_draft(&branch, &path, &markdown, &title, &["postmortem", "needs-human"]).await?)
        }
        None => None,
    };
    let incident = Incident { fingerprint: alert.fingerprint.clone(), facts, detect_seconds, markdown, pr };
    state.scribe.incidents.lock().unwrap_or_else(|e| e.into_inner()).insert(alert.fingerprint, incident);
    Ok(())
}

async fn collect(state: &State, alert: &Alert, tenant: &str) -> Result<(Facts, Option<i64>)> {
    let fired_at = alert.starts_at.as_second();
    let ns = format!("tenant-{tenant}");
    let at = Some(fired_at as u64);
    let mut signals = Vec::new();
    for (name, label, query, unit) in SIGNALS {
        let key = if label == "tenant" { tenant } else { &ns };
        let value = state.sources.at(label, query, at).await?.get(key).copied();
        signals.push((name.to_string(), value.map_or("–".into(), |v| format!("{v:.2}{unit}"))));
    }
    let oom_kills = state.sources.at("namespace", OOM, at).await?.get(&ns).copied().unwrap_or_default();
    let name = alert.labels.get("alertname").cloned().unwrap_or_default();
    let mut timeline = vec![(fired_at, "alertmanager".into(), format!("{name} fired"))];
    let experiments: Vec<(i64, String)> = state
        .runs
        .all()
        .into_iter()
        .filter(|r| r.target == tenant && r.injected_at.is_some_and(|i| i as i64 <= fired_at))
        .filter(|r| r.removed_at.is_none_or(|t| t as i64 >= fired_at - 300))
        .map(|r| (r.injected_at.unwrap_or_default() as i64, r.experiment))
        .collect();
    let detect_seconds = experiments.last().map(|(at, _)| fired_at - at);
    timeline.extend(experiments.iter().map(|(at, e)| (*at, "chaos".into(), format!("{e} injected"))));
    let commits = commits(state, tenant, fired_at).await.unwrap_or_default();
    timeline.extend(commits.iter().map(|(at, m)| (*at, "git".into(), m.clone())));
    timeline.extend(events(state, &ns, fired_at).await?);
    timeline.extend(logs(state, &ns, fired_at).await.unwrap_or_default());
    timeline.sort_by_key(|(at, ..)| *at);
    timeline.dedup_by(|a, b| a.1 == b.1 && a.2 == b.2);
    let leads = Leads { experiments: &experiments, commits: &commits, oom_kills };
    let facts = Facts {
        alert: name,
        tenant: tenant.into(),
        summary: alert.annotations.get("summary").cloned().unwrap_or_default(),
        runbook: alert.annotations.get("runbook_url").cloned().unwrap_or_default(),
        fired_at,
        resolved_at: None,
        signals,
        timeline,
        causes: postmortem::causes(fired_at, &leads),
    };
    Ok((facts, detect_seconds))
}

async fn events(state: &State, ns: &str, fired_at: i64) -> Result<Vec<(i64, String, String)>> {
    let list = Api::<Event>::namespaced(state.sources.kube.clone(), ns).list(&ListParams::default()).await?;
    Ok(list
        .into_iter()
        .filter_map(|e| {
            let at = e.last_timestamp.map(|t| t.0).or(e.event_time.map(|t| t.0))?.as_second();
            let text = format!("{} {}: {}", e.involved_object.name?, e.reason?, e.message?);
            (at >= fired_at - 1800).then_some((at, "kubernetes".into(), text))
        })
        .collect())
}

async fn logs(state: &State, ns: &str, fired_at: i64) -> Result<Vec<(i64, String, String)>> {
    let query = format!(r#"{{namespace="{ns}",level=~"(?i)error"}}"#);
    let start = ((fired_at - 900) * 1_000_000_000).to_string();
    let body: Value = state
        .sources
        .http
        .get(format!("{}/loki/api/v1/query_range", state.scribe.loki))
        .query(&[("query", query.as_str()), ("start", &start), ("limit", "10")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let lines = body["data"]["result"].as_array().into_iter().flatten();
    Ok(lines
        .flat_map(|s| s["values"].as_array().cloned().unwrap_or_default())
        .filter_map(|v| {
            let at = v[0].as_str()?.parse::<i64>().ok()? / 1_000_000_000;
            let line: Value = serde_json::from_str(v[1].as_str()?).unwrap_or_default();
            let text = line["fields"]["message"].as_str().unwrap_or(v[1].as_str()?);
            Some((at, "loki".into(), text.chars().take(160).collect()))
        })
        .collect())
}

async fn commits(state: &State, tenant: &str, fired_at: i64) -> Result<Vec<(i64, String)>> {
    let since = Timestamp::from_second(fired_at - 86400)?.to_string();
    let body: Value = state
        .sources
        .http
        .get(format!("{}/repos/{}/commits", state.scribe.api, state.scribe.repo))
        .header("user-agent", "blastradius")
        .query(&[("path", format!("tenants/{tenant}")), ("since", since)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(body
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let at = c["commit"]["committer"]["date"].as_str()?.parse::<Timestamp>().ok()?;
            let message = c["commit"]["message"].as_str()?.lines().next()?;
            Some((at.as_second(), format!("{} {message}", &c["sha"].as_str()?[..7])))
        })
        .collect())
}
