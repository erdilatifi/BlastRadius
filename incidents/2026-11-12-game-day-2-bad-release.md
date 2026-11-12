# Game day 2: bad release

**Staged on purpose.** A real buggy build was rolled to one tenant to practise detection and rollback. Run [43245613328](https://github.com/erdilatifi/BlastRadius/actions/runs/43245613328), 2026-11-12, all times UTC.

## Summary

acme was rolled to `sha-c0d9171`, a build that panics on single-item quotes but answers its health probes. The rollout completed, every synthetic request to acme failed, and `TenantAvailabilityFastBurn` fired 85 seconds after the last healthy pod left. The Scribe drafted the post-mortem and the remediation playbook chose a revert. The alert resolved 6 minutes after the revert reached `main`.

## Impact

- acme: every request with `items=1` failed from 15:23:26 until the revert rolled out. Error ratio over 5 minutes at firing: 40.85%. Burn rate over 1 hour: 81.7x. The 30-day error budget was spent.
- globex and initech: 0% errors for the whole run.
- Three `pod-kill` chaos runs that overlapped the incident were marked invalid by the scorer, because acme's baseline error ratio was 29% to 100%. That is the scorer working: it refuses to score against a broken baseline.

## Detection

`TenantAvailabilityFastBurn` on acme, from the ingress error ratio. Pod health showed nothing: 0 restarts, all pods Ready.

| Measured from | To the alert |
|---|---|
| The commit reaching `main` (15:22:32) | 2 min 19 s |
| The last healthy pod removed (15:23:26) | 85 s |

`TenantAvailabilitySlowBurn` fired at the same moment.

## Timeline

| Time | Event |
|---|---|
| 15:13:48 | `4aa5225`: every tenant on `sha-e2852c0`, healthy |
| 15:22:32 | `be61bd1` pushed: acme rolled to `sha-c0d9171` |
| 15:23:19 | ArgoCD starts the rollout; the new pods pass `/readyz` |
| 15:23:26 | Last `sha-e2852c0` pod removed |
| 15:24:05 | The campaign's `pod-kill` hits acme (unrelated, overlapping) |
| 15:24:51 | `TenantAvailabilityFastBurn` and `TenantAvailabilitySlowBurn` fire |
| 15:24:51 | Scribe drafts the post-mortem; remediation, in dry-run mode, reports `would open "Revert acme to sha-e2852c0"` |
| 15:34:38 | `ae28a7b` pushed: the revert the playbook chose |
| 15:40:36 | `TenantAvailabilityFastBurn` resolves, 15 min 45 s after firing |

## Root cause

`c0d9171` added a bulk discount: `base - base / (100 / (items - 1).min(100))`. With one item the divisor is `100 / 0`, the handler panics and the connection drops. Synthetic traffic and most real quotes ask for one item.

Two gaps let it reach a tenant:

- `/readyz` returned a constant, so a pod that could not price anything was still Ready and the rollout replaced every healthy pod.
- `release.yml` built, scanned and signed the image without ever calling it.

## Fix

Revert `tenants/acme/values.yaml` to `sha-e2852c0` (`ae28a7b`). ArgoCD rolled acme back.

The revert was pushed by a script standing in for a human merging the bot's pull request. Remediation runs in dry-run mode here because no bot token is configured; it logged the pull request it would have opened.

## Prevention

Merged in `94c967c`:

- **Smoke test before signing.** `release.yml` starts the built image and calls `/readyz`, `/api/quote`, `/api/quote?items=1` and `/api/quote?items=100`. An image that fails is never pushed or signed.
- **Readiness that prices a quote.** `/readyz` now computes `price(1)`. A build with this bug would never become Ready, so the rollout would stall with the old pods still serving.

Proof: draft pull request [#3](https://github.com/erdilatifi/BlastRadius/pull/3) replays the same bug on top of the smoke test. The workload image job fails with `curl: (52) Empty reply from server` in run [43190478833](https://github.com/erdilatifi/BlastRadius/actions/runs/43190478833).

## Runbook

[Tenant availability burn](../runbooks/tenant-availability-burn.md). Step 3, recent changes, points straight at `be61bd1`.

## Lessons

- A probe that does not exercise the request path only proves the process is running.
- The signal that caught this was the SLO at the ingress. Nothing at pod level moved.
- The campaign's `pod-kill` hit acme 46 seconds before the alert, so the Scribe's draft lists two suspects and the ledger's 50 s detection time for that run is measured from the pod kill, not from the release. A staged release should not share a window with the campaign.
- A revert is a safe automatic choice only inside a short window after a release. The alert fired 2 min 25 s after the release commit, well inside the playbook's 15-minute window.
