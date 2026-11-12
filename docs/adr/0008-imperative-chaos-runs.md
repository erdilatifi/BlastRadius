# ADR-0008: Chaos runs are imperative, the catalog is GitOps

**Status:** accepted

## Decision

- The experiment catalog, thresholds and phase durations live in `platform/chaos/catalog.yaml`. They reach the control plane through the `blastradius` Application, so every change is reviewed and versioned like any other config.
- A run is started through the control plane API (`POST /api/runs`, `brctl chaos run`, the console's Run button). The control plane creates the Chaos Mesh object, or calls the workload's fault endpoint, and deletes it when the inject phase ends.
- Guardrails are enforced in three places:

| Layer | Stops |
|---|---|
| Control plane | Unknown experiments; targets whose namespace is not `tenant-<name>` with the `tenant` label and the `chaos-mesh.org/inject: enabled` annotation; a second run while one is active |
| Chaos Mesh | Any namespace without the opt-in annotation (`enableFilterNamespace`); every object carries a duration, so a crashed control plane cannot leave a fault behind |
| RBAC | The control plane may create and delete three chaos kinds and nothing else that changes workloads |

- Every finished run, including invalid and failed ones, goes to the ledger (`data/runs.jsonl`) with the commit and thresholds it was scored with.

## Why

- A run is an operation with a start and an end, like restarting a pod. Desired state in Git would have to be added and then removed, and ArgoCD would fight both edits with self-heal; a failed removal would leave a fault running.
- The catalog is what deserves review: which faults exist, how hard they hit and what counts as impact. Keeping it in Git gives that review without making each run a pull request.
- Scoring needs exact inject and removal times. The process that injects the fault is the one that knows them.

## Alternatives rejected

- **Chaos Mesh `Schedule` or `Workflow` objects in Git.** Runs on a timer whether or not the baseline is healthy, and the results would still need an orchestrator to score them.
- **A pull request per run.** Minutes of CI and review for something that lasts six minutes and is undone automatically.
- **The console talking to Chaos Mesh directly.** Moves the opt-in check into the browser and needs cluster credentials there.
