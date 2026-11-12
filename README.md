# BlastRadius

A multi-tenant Kubernetes platform, managed entirely through Git, that attacks itself every night with chaos experiments, proves that one tenant cannot hurt another, and drafts its own post-incident reviews.

## What it is

- **GitOps platform.** Terraform builds a k3d cluster and bootstraps ArgoCD; every other change is a pull request.
- **Isolated tenants.** One file per tenant becomes a namespace with quotas, network rules, RBAC, Pod Security and Kyverno admission.
- **Nightly chaos.** A scheduled workflow rebuilds the platform from nothing, runs a chaos campaign and scores how many bystander tenants moved.
- **Incident Scribe.** Burn-rate alerts turn into draft post-mortems full of evidence, and guard-railed fix PRs that wait for a human.

It costs $0: clusters run in GitHub Codespaces and GitHub Actions.

## Scorecard

<!-- scorecard:start -->
6 chaos runs: 3 scored, 3 invalid, 0 failed. Last run 2026-11-12 15:45 UTC.

| Experiment | Runs | Runs with impacted bystanders | Most impacted | Detect (median) | Recover (median) |
|---|---|---|---|---|---|
| `cpu-stress` | 3 | 0 | 0 | 5 s | 10 s |
| `pod-kill` | 3 | 0 | – | 50 s | 5 s |
<!-- scorecard:end -->

## Develop

Open the repo in a Codespace, then run `just doctor` to see every pinned tool and `just check` to run the tests. Pinned versions live in [versions.env](versions.env); code rules in [CONTRIBUTING.md](CONTRIBUTING.md); decisions in [docs/adr](docs/adr).
