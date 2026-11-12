# Game day 1: noisy neighbour

**Staged on purpose.** One tenant was given a tier with no CPU limit and no quota on a branch, then put under CPU stress, to see whether its neighbours would feel it. Branch `gameday/noisy-neighbour`, draft pull request [#4](https://github.com/erdilatifi/BlastRadius/pull/4), 2026-11-12, all times UTC.

## Summary

With acme unbounded, `cpu-stress` on acme slowed tenants that were not touched: globex's p95 went from 5 ms to between 17 and 61 ms, initech's from 5 ms to as much as 77 ms. The same experiment on `main`, where every tier has a limit and a quota, moved no neighbour at all. No alert fired; the blast-radius score is what caught it.

## Impact

| Run | acme's tier | globex p95 | initech p95 | Blast radius |
|---|---|---|---|---|
| [43248055167](https://github.com/erdilatifi/BlastRadius/actions/runs/43248055167) | unbounded | 5 → 61 ms | 5 → 77 ms | not scored |
| [43252527797](https://github.com/erdilatifi/BlastRadius/actions/runs/43252527797) | unbounded | 5 → 17 ms | clean | 1 |
| [43245613328](https://github.com/erdilatifi/BlastRadius/actions/runs/43245613328) | small (200m limit) | 4.8 → 4.8 ms | 4.8 → 4.8 ms | 0 |

The first run was marked invalid by the scorer because acme restarted or was unready during the baseline, so it has no score; the neighbours' latencies in that row are still what was measured. No requests failed in any run.

In the bounded run acme itself went from 4.8 to 18 ms. That is the design working: the stressed tenant is throttled at its own limit and pays for its own load.

## Detection

No SLO alert fired. The neighbours' p95 stayed far inside their latency objectives of 250 and 300 ms, so nothing paged. The chaos scorer flagged it, because it compares every bystander's p95 during the fault with its own baseline and counts a tenant as impacted above 1.5 times and 10 ms.

## Timeline

| Time | Event |
|---|---|
| 15:14 | Draft pull request #4 opened: acme on the `unbounded` tier |
| 15:27 | CI on the pull request refuses it (see Prevention) |
| 15:35 | Run 43248055167 starts its campaign on a cluster following the branch |
| 15:37 | `cpu-stress` injected into acme; both neighbours' p95 rise |
| 15:59 | Run 43252527797, a repeat, starts its campaign |
| 16:01 | `cpu-stress` injected into acme; globex impacted, blast radius 1, recovered 20 s after the fault was removed |

## Root cause

The staged tier rendered acme's pods with no CPU limit and its namespace with no ResourceQuota or LimitRange. `cpu-stress` runs two workers at full load in every pod of the target; with two acme pods that is four busy workers on a 4-vCPU runner. Nothing capped them, so they took CPU time the other tenants and the ingress needed.

Admission would normally have stopped the pods. The branch also switched `require-requests-limits` from Deny to Audit so the failure could be staged.

## Fix

Nothing to fix on `main`: the unbounded tier never left its branch. The pull request is closed without merging.

## Prevention

A limitless tier now has to get past four independent checks, and the staged pull request failed three of them in run [43248066131](https://github.com/erdilatifi/BlastRadius/actions/runs/43248066131):

- **CI, manifests job** (`501b449`, added for this game day): every tenant must render a ResourceQuota and a CPU limit. Result: `acme has no ResourceQuota`.
- **CI, Trivy config scan:** `KSV-0011` and `KSV-0018`, container without CPU and memory limits.
- **Tier parsing:** the tier type refuses unknown fields, so a tier cannot grow an opt-out flag without a code change. The first version of the staged tier failed the Rust tests this way.
- **Admission:** Kyverno `require-requests-limits` in Deny refuses any tenant pod without requests and limits.

## Runbook

[Tenant latency burn](../runbooks/tenant-latency-burn.md) for the neighbour's side. When several tenants slow down at once, look for one tenant using far more CPU than its tier allows.

## Lessons

- Limits are what make a neighbour's bad day its own. The same stress cost the bounded tenant 13 ms and its neighbours nothing.
- An SLO alert does not see this kind of harm until it is large. A 12-fold rise in p95 stayed inside the objective. The blast-radius score measures movement, not thresholds.
- The effect varies from run to run (17 to 77 ms) on a shared runner. The direction did not.
- A guard that can be switched to Audit on a branch is only a guard if CI also checks the rendered output.
