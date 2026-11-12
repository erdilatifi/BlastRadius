mod api;
mod sources;
mod traffic;

use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::Router;
use axum::routing::get;
use brcore::{Amount, Tier};
use metrics_exporter_prometheus::PrometheusBuilder;
use serde::Deserialize;
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};

use crate::sources::Sources;

struct Config {
    prometheus: String,
    traefik: String,
    platform: PathBuf,
    web: PathBuf,
}

impl Config {
    fn from_env() -> Config {
        let var = |key: &str, default: &str| env::var(key).unwrap_or_else(|_| default.into());
        Config {
            prometheus: var(
                "PROMETHEUS_URL",
                "http://prometheus-operated.monitoring:9090",
            ),
            traefik: var("TRAEFIK_URL", "http://traefik.kube-system"),
            platform: var("PLATFORM_FILE", "/etc/blastradius/platform.yaml").into(),
            web: var("WEB_DIR", "/web").into(),
        }
    }
}

#[derive(Deserialize)]
pub struct Platform {
    tiers: BTreeMap<String, Tier>,
    capacity: Amount,
}

pub struct State {
    sources: Sources,
    platform: Platform,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().json().init();
    let config = Config::from_env();
    let platform = std::fs::read_to_string(&config.platform)
        .with_context(|| format!("read {}", config.platform.display()))?;
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let state = Arc::new(State {
        sources: Sources::new(http.clone(), config.prometheus).await?,
        platform: serde_saphyr::from_str(&platform)?,
    });
    let metrics = PrometheusBuilder::new().install_recorder()?;
    tokio::spawn(traffic::run(state.clone(), http, config.traefik));

    let web = ServeDir::new(&config.web).fallback(ServeFile::new(config.web.join("index.html")));
    let app = Router::new()
        .route("/api/tenants", get(api::tenants))
        .route("/api/capacity", get(api::capacity))
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
