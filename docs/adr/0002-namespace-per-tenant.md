# ADR-0002: Namespace per tenant

**Status:** accepted

## Decision

Each tenant gets one namespace, `tenant-<name>`, in a shared cluster. Isolation comes from five layers: ResourceQuota and LimitRange, default-deny NetworkPolicies, namespaced RBAC, Pod Security `restricted`, and Kyverno admission rules. Each layer has an automated test.

One Helm chart renders both the tenant's workload and its guardrails, because every tenant here runs the same demo service. In a real organisation tenants would bring their own chart and the platform would own only the guardrails.

## Why

- A namespace is the unit Kubernetes already scopes quotas, network rules, RBAC and Pod Security to.
- Onboarding stays one file and one pull request.
- Many tenants fit in a small cluster; a cluster per tenant would not fit in 16 GB.
- Proving that one tenant cannot hurt another is the point of the project. That question only exists when tenants share a cluster.

## Alternatives rejected

- **Cluster per tenant.** Strongest isolation, but each cluster costs its own control plane and add-ons, and there would be no noisy-neighbour problem to measure.
- **Virtual clusters (vcluster).** Better API isolation, but adds a component per tenant and hides the guardrails this project wants to show.
