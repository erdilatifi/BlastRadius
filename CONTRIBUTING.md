# Contributing

These rules apply to every file type.

1. **No comments.** Not in Rust, TypeScript, Python, YAML, HCL, Dockerfiles or scripts. Names and types say *what*; ADRs and commit messages say *why*. Only exception: the version tag Dependabot keeps next to a pinned action SHA.
2. **Nothing without a purpose.** No dead code, no unused dependency, flag, config key or file. A pull request that adds a dependency says what its one job is.
3. **Short and direct.** Functions do one thing. No abstraction with a single implementation, no "utils" files, no code written for a future that is not planned.
4. **Pure logic apart from I/O.** Scoring, rendering, validation, playbook matching and capacity maths take data and return data, so they are testable without a cluster.
5. **One error path.** `anyhow` and `?` to the edge; no custom error hierarchies; no `unwrap` outside tests.
6. **Config once.** Environment variables parsed into one struct at startup.
7. **Tools decide style.** `rustfmt`, `clippy` with warnings as errors, `biome`, `ruff`, `terraform fmt`. No style debates.
8. **Tests only where logic can be wrong.** We do not test frameworks.

## Size budget

Hand-written lines, excluding tests. If a part wants to exceed its budget, first ask what can be removed.

| Part | Budget |
|---|---|
| `workload` | 200 |
| `core` | 450 |
| `controlplane` | 1,200 |
| `brctl` | 300 |
| `web` | 900 |
| `reports` | 300 |
| Tenant and control-plane charts | 450 |
| Terraform | 200 |
| Scripts | 200 |
| **Whole repo** | **under ~4,500** |

## Checks

`just check` runs the same checks as CI: `rust`, `web` and `reports`.
