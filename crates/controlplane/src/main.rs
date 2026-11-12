mod api;
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
use brcore::{Amount, Tier};
use metrics_exporter_prometheus::PrometheusBuilder;
use serde::Deserialize;
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};

use crate::runs::Runs;
use crate::scribe::Scribe;
use crate::sources::Sources;

struct Config {
    prometheus: String,
    traefik: String,
    platform: PathBuf,
    web: PathBuf,
    commit: String,
    scribe: Scribe,
}

impl Config {
    fn from_env() -> Config {
        let var = |key: &str, default: &str| env::var(key).unwrap_or_else(|_| default.into());
        Config {
            prometheus: var("PROMETHEUS_URL", "http://prometheus-operated.monitoring:9090"),
            traefik: var("TRAEFIK_URL", "http://traefik.kube-system"),
            platform: var("PLATFORM_FILE", "/etc/blastradius/platform.yaml").into(),
            web: var("WEB_DIR", "/web").into(),
            commit: var("COMMIT", "unknown"),
            scribe: Scribe {
                loki: var("LOKI_URL", "http://loki.monitoring:3100"),
                api: var("GITHUB_API", "https://api.github.com"),
                repo: var("GITHUB_REPO", "erdilatifi/BlastRadius"),
                token: env::var("GITHUB_TOKEN")
                    .ok()
                    .filter(|t| !t.is_empty() && var("SCRIBE_MODE", "artifact") == "pr"),
                incidents: Default::default(),
            },
        }
    }
}

#[derive(Deserialize)]
pub struct Platform {
    tiers: BTreeMap<String, Tier>,
    capacity: Amount,
    chaos: runs::Config,
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
    let config = Config::from_env();
    let platform =
        std::fs::read_to_string(&config.platform).with_context(|| format!("read {}", config.platform.display()))?;
    let http = reqwest::Client::builder().timeout(Duration::from_secs(5)).build()?;
    let state = Arc::new(State {
        sources: Sources::new(http.clone(), config.prometheus).await?,
        platform: serde_saphyr::from_str(&platform)?,
        runs: Runs::new(config.commit.clone()),
        scribe: config.scribe,
    });
    let metrics = PrometheusBuilder::new().install_recorder()?;
    tokio::spawn(traffic::run(state.clone(), http, config.traefik));

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
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;
    Ok(())
}
