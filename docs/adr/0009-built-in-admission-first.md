# ADR-0009: Built-in admission first, Kyverno for the rest

**Status:** accepted

## Decision

Every rule Kubernetes can enforce on its own is enforced by Kubernetes: Pod Security Admission `restricted` on each tenant namespace (no root, no privilege escalation, no host access, dropped capabilities, seccomp), LimitRange for default and maximum container size, ResourceQuota for the namespace total.

Kyverno enforces only the rules Kubernetes cannot express:

| Policy | Rule |
|---|---|
| `disallow-latest-tag` | Images are pinned to a tag other than `latest` |
| `require-requests-limits` | Every container sets cpu and memory requests and limits |
| `require-tenant-label` | Pods in `tenant-<name>` carry `tenant: <name>` |
| `restrict-image-registry` | Images come from `ghcr.io/erdilatifi/blastradius` |
| `verify-release-signature` | Images carry a keyless cosign signature from `release.yml` on `main` |

The policies are CEL `ValidatingPolicy` resources in `platform/policies`, scoped to namespaces with a `tenant` label, and auto-generated for Deployments so a bad template is refused at the Deployment, not later at its ReplicaSet. `kyverno test tests/admission` checks the first four in CI without a cluster (the signature check needs the registry and Rekor); `tests/isolation.sh` checks the whole admission chain on a real one.

## Why

- Built-in admission has no webhook to be down, slow or misconfigured. Fewer moving parts guard the most dangerous settings.
- Kyverno then stays small enough to read in one sitting, and the privilege rules hold even when Kyverno is down.
- CEL policies run in the API server's own expression language, so they read like the built-in `ValidatingAdmissionPolicy` and could move there.

## Alternatives rejected

- **Kyverno for everything.** One tool, but it duplicates Pod Security and puts every guardrail behind one webhook.
- **Built-in `ValidatingAdmissionPolicy` only.** No webhook at all, but no `kyverno test` for offline checks and no image signature verification.
- **OPA Gatekeeper.** Rego is a second language to learn for four rules.
