use std::time::Duration;

use anyhow::{Result, bail};
use reqwest::{RequestBuilder, Response};
use serde_json::{Value, json};

pub struct Api {
    http: reqwest::Client,
    base: String,
}

async fn ok(request: RequestBuilder) -> Result<Response> {
    let response = request.send().await?;
    let status = response.status();
    if !status.is_success() {
        bail!("{status}: {}", response.text().await?);
    }
    Ok(response)
}

fn text(v: &Value) -> &str {
    v.as_str().unwrap_or_default()
}

fn items(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}

fn seconds(v: &Value) -> String {
    v.as_u64().map_or("-".into(), |s| format!("{s}s"))
}

impl Api {
    pub fn new(base: &str) -> Api {
        Api {
            http: reqwest::Client::new(),
            base: base.trim_end_matches('/').into(),
        }
    }

    async fn get(&self, path: &str) -> Result<Value> {
        Ok(ok(self.http.get(format!("{}{path}", self.base)))
            .await?
            .json()
            .await?)
    }

    pub async fn run(&self, experiment: &str, tenant: &str) -> Result<Value> {
        let body = json!({"experiment": experiment, "tenant": tenant});
        let mut run: Value = ok(self
            .http
            .post(format!("{}/api/runs", self.base))
            .json(&body))
        .await?
        .json()
        .await?;
        let id = text(&run["id"]).to_string();
        let mut phase = String::new();
        loop {
            let now = text(&run["phase"]).to_string();
            if now != phase {
                println!("{id} {now}");
                phase = now;
            }
            if phase == "done" || phase == "failed" {
                break;
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
            run = self.get(&format!("/api/runs/{id}")).await?;
        }
        let score = &run["score"];
        match (score["blastRadius"].as_u64(), text(&score["invalid"])) {
            _ if phase == "failed" => println!("failed: {}", text(&run["error"])),
            (Some(radius), _) => println!(
                "blast radius {radius}, detect {}, recover {}",
                seconds(&run["detectSeconds"]),
                seconds(&run["recoverSeconds"])
            ),
            (None, why) => println!("invalid: {why}"),
        }
        for d in items(&score["tenants"]) {
            let reasons: Vec<&str> = d["reasons"]
                .as_array()
                .iter()
                .flat_map(|r| r.iter().map(text))
                .collect();
            println!(
                "  {:<12} {:<9} {}",
                text(&d["tenant"]),
                text(&d["role"]),
                reasons.join("; ")
            );
        }
        Ok(run)
    }

    pub async fn campaign(&self, mut experiments: Vec<String>) -> Result<bool> {
        if experiments.is_empty() {
            let catalog = self.get("/api/experiments").await?;
            experiments = catalog
                .as_object()
                .iter()
                .flat_map(|c| c.keys().cloned())
                .collect();
        }
        let tenants = items(&self.get("/api/tenants").await?);
        let (mut total, mut failed) = (0, 0);
        for experiment in &experiments {
            for tenant in &tenants {
                total += 1;
                match self.run(experiment, text(&tenant["name"])).await {
                    Ok(run) if run["phase"] == "done" => {}
                    Ok(_) => failed += 1,
                    Err(e) => {
                        eprintln!("{experiment} on {}: {e:#}", text(&tenant["name"]));
                        failed += 1;
                    }
                }
            }
        }
        println!("{total} runs, {failed} failed");
        Ok(true)
    }

    pub async fn ledger(&self, environment: &str, commit: Option<&str>) -> Result<bool> {
        for mut run in items(&self.get("/api/runs").await?) {
            if run["phase"] != "done" && run["phase"] != "failed" {
                continue;
            }
            run["environment"] = json!(environment);
            if let Some(commit) = commit {
                run["commit"] = json!(commit);
            }
            println!("{run}");
        }
        Ok(true)
    }

    pub async fn status(&self) -> Result<bool> {
        let tenants = items(&self.get("/api/tenants").await?);
        println!("TENANT      TIER        SYNC        HEALTH      AVAILABILITY");
        for t in &tenants {
            let cells = ["name", "tier", "sync", "health"].map(|k| format!("{:<12}", text(&t[k])));
            println!("{}{}%", cells.concat(), t["availability"]);
        }
        Ok(tenants
            .iter()
            .all(|t| t["sync"] == "Synced" && t["health"] == "Healthy"))
    }
}
