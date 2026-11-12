# ADR-0004: SLOs measured at the ingress, alerted on burn rate

**Status:** accepted

## Decision

Every tenant gets two SLOs from its `values.yaml`: availability (`slo.availability`, the share of requests that are not 5xx) and latency (95% of requests faster than `slo.latencyMs`). Both are measured from Traefik's metrics, so they count every request the user sent, including those the pod never answered.

The tenant chart renders a `PrometheusRule` per tenant: error ratios over 5m, 30m, 1h and 6h, the error budget, and four alerts.

| Alert | Long window | Short window | Burn rate | Severity |
|---|---|---|---|---|
| Fast burn | 1h | 5m | 14.4 | page |
| Slow burn | 6h | 30m | 6 | ticket |

A burn rate of 14.4 for one hour spends 2% of a 30-day budget (14.4 × 1h / 720h); 6 for six hours spends 5%. The long window proves the burn is real; the short window makes the alert stop soon after the burn does. Every alert carries a `runbook_url`, and `scripts/check-runbooks.sh` fails CI when one is missing.

Alertmanager sends any alert with a `tenant` label to the control plane webhook. Logs go through Alloy to Loki with a `tenant` label; Grafana shows the Tenant SLO, Platform health and Noisy neighbour dashboards.

## Why

- Users feel what the ingress sees. A pod's own metrics miss requests it never got.
- Alerting on the budget instead of a raw error rate pages only when the SLO is in danger, and scales with each tenant's own target.
- Two windows per alert give both precision and a quick reset, without a long `for`.

## Alternatives rejected

- **Static error rate thresholds.** Simple, but either noisy for a 99.5% tenant or blind for a 99.9% one.
- **Sloth or Pyrra to generate the rules.** Fine tools, but twenty lines of Helm do the same with nothing extra to run.
- **The full Grafana LGTM stack with Mimir and Tempo.** More than one small cluster can hold.
