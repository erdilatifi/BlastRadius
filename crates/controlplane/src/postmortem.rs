use std::fmt::Write;

use brcore::RELEASE_WINDOW;
use k8s_openapi::jiff::Timestamp;
use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Facts {
    pub alert: String,
    pub tenant: String,
    pub summary: String,
    pub runbook: String,
    pub fired_at: i64,
    pub resolved_at: Option<i64>,
    pub signals: Vec<(String, String)>,
    pub timeline: Vec<(i64, String, String)>,
    pub causes: Vec<String>,
}

pub struct Leads<'a> {
    pub experiments: &'a [(i64, String)],
    pub commits: &'a [(i64, String)],
    pub oom_kills: f64,
}

pub fn time(at: i64) -> String {
    Timestamp::from_second(at).map_or(at.to_string(), |t| t.strftime("%Y-%m-%d %H:%M:%S").to_string())
}

pub fn causes(fired_at: i64, leads: &Leads) -> Vec<String> {
    let mut out: Vec<String> = leads
        .experiments
        .iter()
        .map(|(at, name)| format!("Chaos experiment `{name}` was injected at {} UTC.", time(*at)))
        .collect();
    out.extend(
        leads
            .commits
            .iter()
            .filter(|(at, _)| (fired_at - RELEASE_WINDOW..=fired_at).contains(at))
            .map(|(at, msg)| format!("Release: a change to the tenant landed at {} UTC: {msg}", time(*at))),
    );
    if leads.oom_kills > 0.0 {
        out.push(format!("Memory: {} container(s) were OOM-killed.", leads.oom_kills));
    }
    if out.is_empty() {
        out.push("No rule matched. Start from the timeline and the runbook.".into());
    }
    out
}

pub fn render(f: &Facts) -> String {
    let mut s = String::new();
    let human = "_For a human to write. Describe systems and decisions, not people._";
    let state = match f.resolved_at {
        Some(at) => format!("resolved {} UTC, {} min later", time(at), (at - f.fired_at) / 60),
        None => "still firing when this draft was written".into(),
    };
    let _ = writeln!(s, "# Post-mortem draft: {} on {}\n", f.alert, f.tenant);
    let _ = writeln!(
        s,
        "Drafted by the Incident Scribe from evidence. Root cause, fix and prevention are left empty on purpose.\n"
    );
    let _ = writeln!(s, "## Summary\n\n{}. Fired {} UTC, {state}.\n", f.summary, time(f.fired_at));
    let _ = writeln!(s, "## Impact at firing\n\n| Signal | Value |\n|---|---|");
    for (name, value) in &f.signals {
        let _ = writeln!(s, "| {name} | {value} |");
    }
    let _ = writeln!(s, "\n## Timeline (UTC)\n\n| Time | Source | Event |\n|---|---|---|");
    for (at, source, text) in &f.timeline {
        let _ = writeln!(s, "| {} | {source} | {} |", time(*at), text.replace('|', "\\|"));
    }
    let _ = writeln!(s, "\n## Suspected cause\n");
    for cause in &f.causes {
        let _ = writeln!(s, "- {cause}");
    }
    let _ = writeln!(s, "\n## Runbook\n\n{}\n", f.runbook);
    let _ = write!(s, "## Root cause\n\n{human}\n\n## Fix\n\n{human}\n\n## Prevention\n\n{human}\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRED: i64 = 1_800_000_000;

    #[test]
    fn rules_name_experiments_recent_releases_and_oom_kills() {
        let experiments = [(FIRED - 120, "error-burst".to_string())];
        let commits = [(FIRED - 60, "Bump acme".to_string()), (FIRED - 3600, "Old change".to_string())];
        let leads = Leads { experiments: &experiments, commits: &commits, oom_kills: 2.0 };
        let found = causes(FIRED, &leads);
        assert_eq!(found.len(), 3);
        assert!(found[0].contains("`error-burst`"));
        assert!(found[1].contains("Bump acme"));
        assert!(found[2].starts_with("Memory"));
    }

    #[test]
    fn says_so_when_no_rule_matches() {
        let leads = Leads { experiments: &[], commits: &[], oom_kills: 0.0 };
        assert!(causes(FIRED, &leads)[0].starts_with("No rule matched"));
    }

    #[test]
    fn renders_the_post_mortem() {
        let facts = Facts {
            alert: "TenantAvailabilityFastBurn".into(),
            tenant: "acme".into(),
            summary: "acme is burning its availability error budget 14.4x too fast".into(),
            runbook: "https://example.com/runbooks/tenant-availability-burn.md".into(),
            fired_at: FIRED,
            resolved_at: Some(FIRED + 600),
            signals: vec![("Error ratio, 5m".into(), "50.0%".into())],
            timeline: vec![
                (FIRED - 120, "chaos".into(), "error-burst injected".into()),
                (FIRED - 30, "loki".into(), "quote failed | injected fault".into()),
                (FIRED, "alertmanager".into(), "TenantAvailabilityFastBurn fired".into()),
            ],
            causes: vec!["Chaos experiment `error-burst` was injected.".into()],
        };
        insta::assert_snapshot!(render(&facts));
    }
}
