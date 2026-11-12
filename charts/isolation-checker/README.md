# isolation-checker

Installs the BlastRadius control plane on a cluster you already run. It injects one fault into one tenant namespace and checks whether any other tenant noticed.

A run has four phases:

1. **Baseline.** Every tenant is measured for `chaos.baselineSeconds`.
2. **Inject.** One Chaos Mesh fault is created in the target namespace and held for the experiment's `injectSeconds`.
3. **Recovery.** The fault is removed. The checker waits up to `chaos.recoverySeconds` for the target's SLIs to return to its baseline.
4. **Scoring.** Every other tenant is compared with its own baseline.

The **blast radius** is the number of bystanders that moved past the thresholds. Zero means the fault stayed inside its tenant.

## What it needs

- **Chaos Mesh**, with its CRDs, to perform the faults.
- **Prometheus**, which must scrape:
  - [kube-state-metrics](https://github.com/kubernetes/kube-state-metrics), the source of pod restarts and readiness;
  - a request metric for each tenant, with request counts, 5xx counts and a latency histogram. It can come from the application, the ingress controller or a service mesh.
- **Traffic during a run.** Every tenant must receive requests in the baseline. A tenant with no requests can't show harm, so the run is reported as invalid instead of scored.

The chart installs a single Deployment and its RBAC. It reads namespaces and pods across the cluster. It can create and delete the Chaos Mesh kinds your catalog uses, in any namespace. The control plane itself only creates a fault in a namespace that has opted in.

## Install

```bash
helm install isolation-checker charts/isolation-checker \
  --namespace isolation-checker --create-namespace \
  --set image.tag=sha-<commit> \
  -f my-values.yaml
```

`image.tag` is required: pick a published `sha-<commit>` tag of `ghcr.io/erdilatifi/blastradius/controlplane`. The control plane validates the tenant config when it starts, and logs the reason if the config is wrong.

## Tell it what a tenant is

A tenant is a namespace that matches `tenants.selector`, which uses Kubernetes label-selector syntax.

- **Name:** the value of the `nameLabel` label, or the namespace name if `nameLabel` is left out. Names must be DNS labels. If two namespaces give the same name, the first one is kept and the second is skipped with a warning.
- **Tier:** optional, taken from the `tierLabel` label.

```bash
kubectl label namespace payments-prod tenant=payments
```

## Write the three queries

`tenants.slis` holds three PromQL templates. Each one runs once per tenant and must return a single series, so wrap it in `sum(...)`. The checker fills in these placeholders:

| Placeholder | Becomes |
| --- | --- |
| `$namespace` | the tenant's namespace |
| `$tenant` | the tenant's name |
| `$range` | the window being measured, such as `120s` |

The queries are:

- **`requests`:** requests per second.
- **`errors`:** failed requests per second. If this returns nothing, the tenant had no errors.
- **`p95Seconds`:** 95th percentile latency in seconds. Divide by 1000 if your histogram is in milliseconds, as Istio's is.

`values.yaml` has working examples for application metrics, ingress-nginx and Istio. If one tenant needs a different query, replace it under `tenants.overrides.<tenant>`.

Check a query before a run: run it in the Prometheus UI with the placeholders filled in by hand.

## Opt a namespace in

Faults go only into namespaces that carry the opt-in annotation. Every tenant is still measured as a bystander, whether or not it has opted in.

```bash
kubectl annotate namespace payments-prod chaos-mesh.org/inject=enabled
```

Use `tenants.targetPods` to narrow the fault to some pods in the namespace, for example `{app.kubernetes.io/name: api}`.

## Run an experiment

The API has no login yet. The NetworkPolicy blocks all traffic to it from inside the cluster, so reach it with a port-forward:

```bash
kubectl -n isolation-checker port-forward svc/isolation-checker 8080
```

```bash
BLASTRADIUS_API=http://localhost:8080 brctl chaos run pod-kill payments
```

`brctl` follows the run and prints the blast radius and the reasons for each tenant. The console at http://localhost:8080 shows the same data.

The experiments are in `chaos.experiments`. Each one is a Chaos Mesh spec without `selector` or `duration`, because the checker fills those in. When you add a new `kind`, the chart's ClusterRole picks it up.

## Optional

| Value | Effect |
| --- | --- |
| `integrations.detectAlert` | A PromQL query, such as `ALERTS{...}`, that returns a series while your alert for the target fires. Records time to detect. |
| `integrations.argocd` | Shows sync and health from Argo CD Applications named `<appPrefix><tenant>`. |
| `networkPolicy.allowedNamespaces` | Namespaces that may reach the API, such as the ingress controller's or Prometheus's. |
| `serviceMonitor.enabled` | Scrapes the control plane's own `/metrics`. |
| `ingress.enabled` | Exposes the console. Put authentication in front of it, because anyone who reaches the API can start a fault. |

## Limits

- **Runs are kept in memory.** They are lost when the pod restarts.
- **One namespace per tenant.**
