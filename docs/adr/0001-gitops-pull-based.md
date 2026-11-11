# ADR-0001: GitOps, pull-based

**Status:** accepted

## Decision

Git is the only way the cluster changes. ArgoCD runs inside the cluster and pulls `main` every 60 seconds. CI builds, tests and signs images, then opens a pull request that bumps the image tag. CI never talks to a cluster.

## Why

- The cluster holds no credentials for anything outside it except Git read access, and CI holds no cluster credentials at all.
- Every change is a reviewed, recorded, reversible commit. Rollback is `git revert`.
- ArgoCD reports drift between Git and the cluster and corrects it.
- Clusters here are rebuilt every night; pointing a new ArgoCD at the repo is the whole restore procedure.

## Alternatives rejected

- **Push from CI with `kubectl` or `helm upgrade`.** CI would need cluster admin credentials, and nothing would notice drift. A Codespace cluster is also not reachable from a hosted runner.
- **Flux.** Equally pull-based; ArgoCD's ApplicationSet and UI fit the tenant fan-out and the demo better.
