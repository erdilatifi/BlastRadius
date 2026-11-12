use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use anyhow::{Context, bail};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Json, Router};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::net::TcpListener;

const MAX_ITEMS: u32 = 1000;
const ROUNDS_PER_ITEM: u32 = 20_000;
const DIAL_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Fault {
    error_rate: f64,
    latency_ms: u64,
    leak_kib: u64,
}

#[derive(Default)]
struct Inner {
    fault: Fault,
    requests: u64,
    leaked: Vec<Vec<u8>>,
}

#[derive(Clone, Default)]
struct App(Arc<Mutex<Inner>>);

impl App {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn begin_quote(&self) -> (Fault, u64) {
        let mut inner = self.lock();
        let n = inner.requests;
        inner.requests += 1;
        let fault = inner.fault;
        if fault.leak_kib > 0 {
            inner.leaked.push(vec![1; fault.leak_kib as usize * 1024]);
            metrics::gauge!("workload_leaked_bytes").increment(fault.leak_kib as f64 * 1024.0);
        }
        (fault, n)
    }

    fn set_fault(&self, fault: Fault) {
        let mut inner = self.lock();
        inner.fault = fault;
        if fault.leak_kib == 0 {
            inner.leaked = Vec::new();
            metrics::gauge!("workload_leaked_bytes").set(0.0);
        }
    }
}

#[derive(Deserialize)]
struct QuoteQuery {
    #[serde(default = "one")]
    items: u32,
}

fn one() -> u32 {
    1
}

fn price(items: u32) -> u64 {
    (1..=u64::from(items.min(MAX_ITEMS))).fold(0, |total, item| {
        let mut x = item;
        for _ in 0..ROUNDS_PER_ITEM {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
        }
        total + x % 1000
    })
}

fn fails(n: u64, error_rate: f64) -> bool {
    ((n + 1) as f64 * error_rate).floor() > (n as f64 * error_rate).floor()
}

async fn quote(State(app): State<App>, Query(query): Query<QuoteQuery>) -> Response {
    let (fault, n) = app.begin_quote();
    if fault.latency_ms > 0 {
        tokio::time::sleep(Duration::from_millis(fault.latency_ms)).await;
    }
    if fails(n, fault.error_rate) {
        metrics::counter!("workload_quotes_total", "outcome" => "error").increment(1);
        return (StatusCode::INTERNAL_SERVER_ERROR, "injected fault").into_response();
    }
    metrics::counter!("workload_quotes_total", "outcome" => "ok").increment(1);
    Json(json!({ "items": query.items, "total": price(query.items) })).into_response()
}

async fn set_fault(State(app): State<App>, Json(fault): Json<Fault>) -> Response {
    if !(0.0..=1.0).contains(&fault.error_rate) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            "error_rate must be between 0 and 1",
        )
            .into_response();
    }
    app.set_fault(fault);
    tracing::warn!(?fault, "fault set");
    Json(fault).into_response()
}

fn public(app: App) -> Router {
    Router::new()
        .route("/api/quote", get(quote))
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(|| async { "ok" }))
        .with_state(app)
}

fn admin(app: App, metrics: PrometheusHandle) -> Router {
    Router::new()
        .route("/admin/fault", put(set_fault))
        .route("/metrics", get(move || async move { metrics.render() }))
        .with_state(app)
}

fn dial(addr: &str) -> anyhow::Result<()> {
    let Some(addr) = addr.to_socket_addrs()?.next() else {
        bail!("{addr} resolves to no address");
    };
    TcpStream::connect_timeout(&addr, DIAL_TIMEOUT).with_context(|| format!("dial {addr}"))?;
    Ok(())
}

async fn serve() -> anyhow::Result<()> {
    tracing_subscriber::fmt().json().init();
    let metrics = PrometheusBuilder::new().install_recorder()?;
    let app = App::default();
    let public_listener = TcpListener::bind("0.0.0.0:8080").await?;
    let admin_listener = TcpListener::bind("0.0.0.0:9090").await?;
    tracing::info!("listening on 8080 (public) and 9090 (admin)");
    tokio::try_join!(
        axum::serve(public_listener, public(app.clone())).with_graceful_shutdown(stop()),
        axum::serve(admin_listener, admin(app, metrics)).with_graceful_shutdown(stop()),
    )?;
    Ok(())
}

async fn stop() {
    let _ = tokio::signal::ctrl_c().await;
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => tokio::runtime::Runtime::new()?.block_on(serve()),
        [cmd, addr] if cmd == "dial" => dial(addr),
        _ => bail!("usage: workload [dial host:port]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use std::time::Instant;
    use tower::ServiceExt;

    fn routers() -> (App, Router, Router) {
        let app = App::default();
        let handle = PrometheusBuilder::new().build_recorder().handle();
        (app.clone(), public(app.clone()), admin(app, handle))
    }

    async fn put_fault(admin: &Router, body: &str) -> StatusCode {
        let request = Request::put("/admin/fault")
            .header("content-type", "application/json")
            .body(Body::from(body.to_owned()))
            .unwrap();
        admin.clone().oneshot(request).await.unwrap().status()
    }

    async fn get_quote(public: &Router) -> (StatusCode, String) {
        let request = Request::get("/api/quote?items=3")
            .body(Body::empty())
            .unwrap();
        let response = public.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    async fn statuses(public: &Router, count: usize) -> Vec<StatusCode> {
        let mut out = Vec::new();
        for _ in 0..count {
            out.push(get_quote(public).await.0);
        }
        out
    }

    #[tokio::test]
    async fn quote_is_deterministic() {
        let (_, public, _) = routers();
        let (status, body) = get_quote(&public).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "items": 3, "total": price(3) }).to_string());
        assert_eq!(price(MAX_ITEMS + 1), price(MAX_ITEMS));
    }

    #[tokio::test]
    async fn error_rate_on_and_off() {
        let (_, public, admin) = routers();
        assert_eq!(
            put_fault(&admin, r#"{"error_rate":0.5}"#).await,
            StatusCode::OK
        );
        let errors = statuses(&public, 10)
            .await
            .into_iter()
            .filter(|s| *s == StatusCode::INTERNAL_SERVER_ERROR)
            .count();
        assert_eq!(errors, 5);
        assert_eq!(put_fault(&admin, "{}").await, StatusCode::OK);
        assert!(
            statuses(&public, 10)
                .await
                .iter()
                .all(|s| *s == StatusCode::OK)
        );
    }

    #[tokio::test]
    async fn full_error_rate_fails_every_request() {
        let (_, public, admin) = routers();
        put_fault(&admin, r#"{"error_rate":1}"#).await;
        assert!(
            statuses(&public, 5)
                .await
                .iter()
                .all(|s| *s == StatusCode::INTERNAL_SERVER_ERROR)
        );
    }

    #[tokio::test]
    async fn latency_on_and_off() {
        let (_, public, admin) = routers();
        put_fault(&admin, r#"{"latency_ms":200}"#).await;
        let start = Instant::now();
        assert_eq!(get_quote(&public).await.0, StatusCode::OK);
        assert!(start.elapsed() >= Duration::from_millis(200));
        put_fault(&admin, "{}").await;
        let start = Instant::now();
        get_quote(&public).await;
        assert!(start.elapsed() < Duration::from_millis(200));
    }

    #[tokio::test]
    async fn leak_on_and_off() {
        let (app, public, admin) = routers();
        put_fault(&admin, r#"{"leak_kib":256}"#).await;
        statuses(&public, 4).await;
        let leaked: usize = app.lock().leaked.iter().map(Vec::len).sum();
        assert_eq!(leaked, 4 * 256 * 1024);
        put_fault(&admin, "{}").await;
        statuses(&public, 4).await;
        assert!(app.lock().leaked.is_empty());
    }

    #[tokio::test]
    async fn rejects_bad_faults() {
        let (app, _, admin) = routers();
        assert_eq!(
            put_fault(&admin, r#"{"error_rate":1.5}"#).await,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            put_fault(&admin, r#"{"errors":1}"#).await,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(app.lock().fault, Fault::default());
    }

    #[tokio::test]
    async fn admin_routes_are_not_public() {
        let (_, public, _) = routers();
        let request = Request::put("/admin/fault").body(Body::from("{}")).unwrap();
        assert_eq!(
            public.oneshot(request).await.unwrap().status(),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn dial_reaches_open_port_only() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        assert!(dial(&addr).is_ok());
        drop(listener);
        assert!(dial(&addr).is_err());
        assert!(dial("not-an-address").is_err());
    }
}
