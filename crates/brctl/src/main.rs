use std::env;
use std::path::Path;

use anyhow::{Context, Result};
use brcore::{Change, GitHub, Repo, Tenant};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "BlastRadius platform CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(subcommand, about = "Tenant lifecycle; every change is a pull request")]
    Tenant(TenantCommand),
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
    List,
    #[command(about = "Open a pull request removing a tenant")]
    Rm { name: String },
}

fn github(repo: &Repo) -> Result<GitHub> {
    let token = env::var("GITHUB_TOKEN")
        .context("set GITHUB_TOKEN to a token that can open pull requests")?;
    let api = env::var("GITHUB_API_URL").unwrap_or_else(|_| "https://api.github.com".into());
    GitHub::new(&api, &repo.slug, &token)
}

fn path(name: &str) -> String {
    format!("tenants/{name}/values.yaml")
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut repo = Repo::load(Path::new("."))?;
    let Command::Tenant(command) = Cli::parse().command;
    match command {
        TenantCommand::New {
            name,
            tier,
            dry_run,
        } => {
            let tenant = Tenant::new(&name, &tier, repo.tag()?);
            let yaml = tenant.to_yaml()?;
            repo.add(tenant)?;
            if dry_run {
                print!("{yaml}");
                return Ok(());
            }
            let title = format!("Add tenant {name} ({tier})");
            let body = format!(
                "Onboards `{name}` on the `{tier}` tier. Validated and inside the capacity gate."
            );
            let url = github(&repo)?
                .open_pr(
                    &format!("tenant-new-{name}"),
                    &path(&name),
                    Change::Write(yaml),
                    &title,
                    &body,
                )
                .await?;
            println!("{url}");
        }
        TenantCommand::List => {
            repo.check()?;
            println!(
                "{:<20} {:<8} {:>8} {:>8}",
                "TENANT", "TIER", "REPLICAS", "SLO"
            );
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
                used.cpu_millis,
                cap.cpu_millis,
                used.memory_mib,
                cap.memory_mib,
                used.pods,
                cap.pods
            );
        }
        TenantCommand::Rm { name } => {
            repo.remove(&name)?;
            let title = format!("Remove tenant {name}");
            let url = github(&repo)?
                .open_pr(
                    &format!("tenant-rm-{name}"),
                    &path(&name),
                    Change::Delete,
                    &title,
                    &title,
                )
                .await?;
            println!("{url}");
        }
    }
    Ok(())
}
