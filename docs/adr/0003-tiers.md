# ADR-0003: Three fixed tiers

**Status:** accepted

## Decision

A tenant picks one of three tiers: `small`, `medium`, `large`. The tier table lives once, in `charts/tenant/values.yaml`, and sets each container's requests and limits, the namespace ResourceQuota, and the LimitRange defaults and maximum. Tenants cannot set their own numbers.

| Tier | Requests | Limits | Quota |
|---|---|---|---|
| small | 50m, 32Mi | 200m, 64Mi | 400m, 256Mi, 6 pods |
| medium | 100m, 64Mi | 400m, 128Mi | 800m, 512Mi, 8 pods |
| large | 200m, 128Mi | 800m, 256Mi | 1600m, 1024Mi, 12 pods |

Numbers are integers (`cpuMillis`, `memoryMiB`) so `brcore` can do capacity maths without parsing Kubernetes quantities. The capacity gate sums tier quotas and refuses a change that would commit more than 85% of `platform/capacity.yaml`. `brctl` and CI run the same function.

A tenant's `replicas.max` plus one rolling-update surge pod must fit in its tier quota, so validation catches a tenant that could never scale to its own maximum.

## Why

- Quota is what the scheduler and the capacity gate can count. Free-form numbers make every tenant a special case.
- Requests are a quarter of limits: tenants can burst, and the node is sized by requests.
- Three tiers are enough to show a noisy-neighbour difference between tenants and few enough to load-test each one.

## Alternatives rejected

- **Per-tenant numbers in the tenant file.** Flexible, but every review becomes a sizing discussion and the gate no longer has a fixed unit.
- **Kubernetes quantity strings in the table.** Familiar, but needs a quantity parser in Rust for no gain.
