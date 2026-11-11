# ADR-0006: Language lanes

**Status:** accepted

## Decision

Each language has one lane:

| Language | Lane |
|---|---|
| Rust | Everything that runs in or talks to the cluster: workload, control plane, `brctl` |
| TypeScript/React | The console. Displays what the API returns; holds no business logic |
| Python | Offline analysis over data in Git: forecast, weekly report, scorecard. Standard library only |
| Bash | Tests and game days that drive `kubectl` and `docker` |
| PowerShell | The Windows workstation: connect to the Codespace, forward ports |
| HCL, YAML | Infrastructure and desired state |

## Why

- Rust services use about 10 MB per pod, so many tenants fit in a small cluster, and one type system covers the API, the CLI and the shared tenant model.
- The console only renders, so its logic cannot drift from the control plane's.
- Python reads files in Git, never a live cluster, so reports work after the nightly cluster is gone.
- Clear lanes mean no feature is written twice in two languages.

## Alternatives rejected

- **Python everywhere.** Larger pods, and the CLI and services would share a model only by convention.
- **Go.** The Kubernetes default, but Rust's `kube` crate covers everything needed here.
- **Everything in Rust, including reports.** Possible, but number-crunching scripts over JSON lines are shorter in Python.
