use std::sync::Arc;
use std::time::Duration;

use crate::State;

const INTERVAL: Duration = Duration::from_millis(200);
const REFRESH_TICKS: u32 = 50;

pub async fn run(state: Arc<State>, http: reqwest::Client, traefik: String) {
    let url = format!("{traefik}/api/quote");
    let mut tenants = Vec::new();
    let mut tick = tokio::time::interval(INTERVAL);
    let mut n = 0u32;
    loop {
        tick.tick().await;
        n = (n + 1) % REFRESH_TICKS;
        if n == 1 {
            match state.sources.tenants().await {
                Ok(list) => tenants = list,
                Err(e) => tracing::warn!(error = %e, "list tenants"),
            }
        }
        for tenant in &tenants {
            let request = http.get(&url).header("host", format!("{}.blastradius.localhost", tenant.name));
            let name = tenant.name.clone();
            tokio::spawn(async move {
                let outcome = match request.send().await {
                    Ok(r) if r.status().is_success() => "ok",
                    Ok(_) => "error",
                    Err(_) => "unreachable",
                };
                metrics::counter!("controlplane_traffic_requests_total", "tenant" => name, "outcome" => outcome)
                    .increment(1);
            });
        }
    }
}
