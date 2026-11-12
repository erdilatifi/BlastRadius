# ADR-0005: Remediation opens a pull request and stops

**Status:** accepted

## Decision

- The control plane runs two playbooks when a tenant alert fires. Both can only propose a change to `tenants/<name>/values.yaml` as a pull request. Neither merges, and neither touches the cluster.

| Playbook | Trigger | Proposed change |
|---|---|---|
| `revert-release` | `Tenant*FastBurn` or `TenantCrashLooping` within 15 minutes of a commit under `tenants/<name>` that changed `image.tag` | `image.tag` back to the value before that commit |
| `upgrade-tier` | Three or more OOM kills in 30 minutes | The next tier by memory, only if it still fits under 85% of platform capacity |

- Guardrails, all checked before anything is written:

| Guardrail | Prevents |
|---|---|
| Dry run by default (`remediation.mode: dry-run`); PRs only with `pr` and a token | A new install opening PRs before anyone has watched what it would do |
| Tenant must have `remediation: true` | Changes to a tenant whose owner opted out |
| One open `remediate-<name>-*` PR per tenant | A flapping alert stacking up competing fixes |
| At most three PRs per hour across all tenants | A platform-wide incident flooding reviewers |
| The capacity gate on `upgrade-tier` | A fix that breaks the commitment to every other tenant |
| Never merges | The bot being the last word on production |

- The decision, a dry-run title, or the reason nothing was opened, is shown on the incident in the console.
- `scaling blocked by quota` is not a trigger. Tenant validation already makes every tier's quota hold `replicas.max` plus one surge pod, so the case cannot arise from a file in Git.

## Why

- A fix through Git is reviewed, recorded with its reason, and reversible with one more revert. The same ArgoCD sync that shipped the bad change ships the fix.
- The control plane needs no write access to Deployments. A bug or a compromised token can at worst open a pull request, which a human still has to merge.
- Both playbooks change one field the platform already validates, so CI checks the fix like any other change.

## Alternatives rejected

- **Roll back in the cluster** (`kubectl rollout undo`, Argo Rollouts). Fast, but ArgoCD's self-heal would put the bad tag straight back, and Git would no longer describe what runs.
- **Auto-merge the fix PR.** Removes the review that makes a bot safe to run, for a few minutes saved.
- **A general rules engine.** Two playbooks with tests are easier to trust than a language for writing them.
