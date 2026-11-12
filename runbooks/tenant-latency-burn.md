# Tenant latency burn

`TenantLatencyFastBurn` (page) or `TenantLatencySlowBurn` (ticket): too many requests at the tenant's ingress take longer than its `slo.latencyMs`. The objective is that 95% of requests are faster than that.

## Check
1. Grafana, **Tenant SLO** for the burn, then **Noisy neighbour**: is the tenant's CPU throttled, or is another tenant on the same node using a lot of CPU?
2. Is p95 high for every tenant? Then look at the node and Traefik, not the tenant.
3. Recent changes under `tenants/<name>` and `kubectl -n tenant-<name> get hpa`: is the HPA at its maximum?
4. Logs: in Grafana Explore, Loki, `{tenant="<name>"}` around the start of the burn.

## Act
- CPU throttled at the HPA maximum: raise `replicas.max` or the tier with a PR.
- An injected fault: clear `latency_ms` with `PUT /admin/fault` on each pod's port 9090.
- A noisy neighbour: check its quota and limits; the guardrails should cap it, so a breach is a platform bug.

## Close
The alert resolves once the short window drops under the threshold. Note the cause and the remaining budget in the incident.
