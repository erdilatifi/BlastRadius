mod chaos;

use std::env;
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result};
use brcore::{Change, GitHub, Repo, Tenant};
use clap::{Parser, Subcommand};

const API: &str = "http://localhost:8080";

#[derive(Parser)]
#[command(about = "BlastRadius platform CLI")]
struct Cli {
    #[arg(long, global = true, env = "BLASTRADIUS_API", default_value = API)]
    api: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(subcommand, about = "Tenant lifecycle; every change is a pull request")]
    Tenant(TenantCommand),
    #[command(subcommand, about = "Chaos runs against tenants that opted in")]
    Chaos(ChaosCommand),
    #[command(subcommand, about = "The run ledger")]
    Ledger(LedgerCommand),
    #[command(about = "Tenant sync and health; fails unless every tenant is Synced and Healthy")]
    Status,
}

#[derive(Subcommand)]
enum ChaosCommand {
    #[command(about = "Start one run and follow it to its score")]
    Run { experiment: String, tenant: String },
    #[command(about = "Run catalog experiments (default all) against every tenant in turn")]
    Campaign { experiments: Vec<String> },
}

#[derive(Subcommand)]
enum LedgerCommand {
    #[command(about = "Print this cluster's finished runs as JSON lines")]
    Export {
        #[arg(long, default_value = "local")]
        environment: String,
        #[arg(long)]
        commit: Option<String>,
    },
}

#[derive(Subcommand)]
enum TenantCommand {
    #[command(about = "Validate, run the capacity gate and open a pull request adding a tenant")]
    New {
        name: String,
        #[arg(long, default_value = "small")]
        tier: String,
        #[arg(long, help = "Print the tenant file instead of opening a pull request")]
        dry_run: bool,
    },
    #[command(about = "List tenants and check them against the capacity gate")]
    List {
        #[arg(long, help = "Print tiers, capacity and tenants as JSON for the reports")]
        json: bool,
    },
    #[command(about = "Open a pull request removing a tenant")]
    Rm { name: String },
}

fn github(repo: &Repo) -> Result<GitHub> {
    let token = env::var("GITHUB_TOKEN").context("set GITHUB_TOKEN to a token that can open pull requests")?;
    let api = env::var("GITHUB_API_URL").unwrap_or_else(|_| "https://api.github.com".into());
    GitHub::new(&api, &repo.slug, &token)
}

fn path(name: &str) -> String {
    format!("tenants/{name}/values.yaml")
}

#[tokio::main]
async fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let api = chaos::Api::new(&cli.api);
    let ok = match cli.command {
        Command::Tenant(command) => tenant(command).await.map(|_| true)?,
        Command::Chaos(ChaosCommand::Run { experiment, tenant }) => {
            api.run(&experiment, &tenant).await?["phase"] == "done"
        }
        Command::Chaos(ChaosCommand::Campaign { experiments }) => api.campaign(experiments).await?,
        Command::Ledger(LedgerCommand::Export { environment, commit }) => {
            api.ledger(&environment, commit.as_deref()).await?
        }
        Command::Status => api.status().await?,
    };
    Ok(ExitCode::from(u8::from(!ok)))
}

async fn tenant(command: TenantCommand) -> Result<()> {
    let mut repo = Repo::load(Path::new("."))?;
    match command {
        TenantCommand::New { name, tier, dry_run } => {
            let tenant = Tenant::new(&name, &tier, repo.tag()?);
            let yaml = tenant.to_yaml()?;
            repo.add(tenant)?;
            if dry_run {
                print!("{yaml}");
                return Ok(());
            }
            let title = format!("Add tenant {name} ({tier})");
            let body = format!("Onboards `{name}` on the `{tier}` tier. Validated and inside the capacity gate.");
            let url = github(&repo)?
                .open_pr(&format!("tenant-new-{name}"), &path(&name), Change::Write(yaml), &title, &body)
                .await?;
            println!("{url}");
        }
        TenantCommand::List { json } => {
            repo.check()?;
            if json {
                println!("{}", serde_json::to_string(&repo)?);
                return Ok(());
            }
            println!("{:<20} {:<8} {:>8} {:>8}", "TENANT", "TIER", "REPLICAS", "SLO");
            for t in &repo.tenants {
                println!(
                    "{:<20} {:<8} {:>8} {:>8}",
                    t.name,
                    t.tier,
                    format!("{}-{}", t.replicas.min, t.replicas.max),
                    t.slo.availability
                );
            }
            let (used, cap) = (repo.committed(), repo.capacity);
            println!(
                "committed {}m/{}m cpu, {}Mi/{}Mi memory, {}/{} pods (gate 85%)",
                used.cpu_millis, cap.cpu_millis, used.memory_mib, cap.memory_mib, used.pods, cap.pods
            );
        }
        TenantCommand::Rm { name } => {
            repo.remove(&name)?;
            let title = format!("Remove tenant {name}");
            let url = github(&repo)?
                .open_pr(&format!("tenant-rm-{name}"), &path(&name), Change::Delete, &title, &title)
                .await?;
            println!("{url}");
        }
    }
    Ok(())
}
