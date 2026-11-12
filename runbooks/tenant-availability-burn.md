# Tenant availability burn

`TenantAvailabilityFastBurn` (page) or `TenantAvailabilitySlowBurn` (ticket): more requests to the tenant's ingress return 5xx than its availability SLO allows. Fast burn spends 2% of a 30-day budget in an hour; slow burn spends 5% in six hours.

## Check
1. Grafana, **Tenant SLO**, pick the tenant: is the burn still rising or already falling?
2. Is it one tenant or many? Many at once points at the platform (Traefik, a node), not the tenant.
3. Recent changes: `kubectl -n argocd get application tenant-<name>` and the last commits under `tenants/<name>`.
4. Logs: in Grafana Explore, Loki, `{tenant="<name>"} | level="ERROR"`.
5. Pods: `kubectl -n tenant-<name> get pods` for restarts, OOMKilled or Pending.

## Act
- A bad rollout: revert the commit in `tenants/<name>`; ArgoCD rolls back.
- An injected fault: `PUT /admin/fault` with `{"error_rate":0,"latency_ms":0,"leak_kib":0}` on each pod's port 9090.
- Out of capacity: Pending pods or a full quota; move the tenant up a tier with a PR.

## Close
The alert resolves on its own once the short window drops under the threshold. Note what happened and the remaining budget in the incident.
