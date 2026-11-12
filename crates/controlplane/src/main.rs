mod api;
mod config;
mod postmortem;
mod runs;
mod score;
mod scribe;
mod sources;
mod traffic;

use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::Router;
use axum::routing::{get, post};
use brcore::{Amount, GitHub, Tier};
use metrics_exporter_prometheus::PrometheusBuilder;
use serde::Deserialize;
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};

use crate::runs::Runs;
use crate::scribe::Scribe;
use crate::sources::Sources;

struct Config {
    prometheus: String,
    platform: PathBuf,
    web: PathBuf,
    commit: String,
    scribe: Scribe,
}

impl Config {
    fn from_env() -> Result<Config> {
        let var = |key: &str, default: &str| env::var(key).unwrap_or_else(|_| default.into());
        let token = var("GITHUB_TOKEN", "");
        let pr_mode = |key: &str| !token.is_empty() && var(key, "") == "pr";
        Ok(Config {
            prometheus: var("PROMETHEUS_URL", "http://prometheus-operated.monitoring:9090"),
            platform: var("PLATFORM_FILE", "/etc/blastradius/platform.yaml").into(),
            web: var("WEB_DIR", "/web").into(),
            commit: var("COMMIT", "unknown"),
            scribe: Scribe {
                loki: var("LOKI_URL", "http://loki.monitoring:3100"),
                github: GitHub::new(
                    &var("GITHUB_API", "https://api.github.com"),
                    &var("GITHUB_REPO", "erdilatifi/BlastRadius"),
                    &token,
                )?,
                drafts: pr_mode("SCRIBE_MODE"),
                fixes: pr_mode("REMEDIATION_MODE"),
                incidents: Default::default(),
                opened: Default::default(),
            },
        })
    }
}

#[derive(Deserialize)]
pub struct Platform {
    #[serde(default)]
    tiers: BTreeMap<String, Tier>,
    #[serde(default)]
    capacity: Amount,
    chaos: runs::Config,
    tenants: config::Tenants,
    #[serde(default)]
    integrations: config::Integrations,
}

pub struct State {
    sources: Sources,
    platform: Platform,
    runs: Runs,
    scribe: Scribe,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().json().init();
    let config = Config::from_env()?;
    let platform =
        std::fs::read_to_string(&config.platform).with_context(|| format!("read {}", config.platform.display()))?;
    let platform: Platform =
        serde_saphyr::from_str(&platform).with_context(|| format!("parse {}", config.platform.display()))?;
    platform.tenants.validate()?;
    let http = reqwest::Client::builder().timeout(Duration::from_secs(5)).build()?;
    let sources =
        Sources::new(http.clone(), config.prometheus, platform.tenants.clone(), platform.integrations.argocd.clone())
            .await?;
    let traffic = platform.integrations.traffic.clone();
    let state = Arc::new(State { sources, platform, runs: Runs::new(config.commit.clone()), scribe: config.scribe });
    let metrics = PrometheusBuilder::new().install_recorder()?;
    if let Some(traffic) = traffic {
        tokio::spawn(traffic::run(state.clone(), http, traffic));
    }

    let web = ServeDir::new(&config.web).fallback(ServeFile::new(config.web.join("index.html")));
    let app = Router::new()
        .route("/api/tenants", get(api::tenants))
        .route("/api/capacity", get(api::capacity))
        .route("/api/experiments", get(api::experiments))
        .route("/api/runs", get(api::runs).post(api::start_run))
        .route("/api/runs/{id}", get(api::run))
        .route("/api/incidents", get(scribe::incidents))
        .route("/hooks/alertmanager", post(scribe::hook))
        .route("/metrics", get(move || async move { metrics.render() }))
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(|| async { "ok" }))
        .fallback_service(web)
        .with_state(state);
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("listening on 8080");
    axum::serve(listener, app).await?;
    Ok(())
}
