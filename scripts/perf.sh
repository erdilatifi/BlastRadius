#!/usr/bin/env bash
set -euo pipefail

out=$1
shift
cd "$(dirname "$0")/.."
kubectl -n kube-system port-forward svc/traefik 8084:80 >/dev/null &
kubectl -n monitoring port-forward svc/prometheus-operated 9092:9090 >/dev/null &
trap 'kill $(jobs -p)' EXIT
sleep 3
prom() { curl -s localhost:9092/api/v1/query --data-urlencode "query=$1" | jq -r '.data.result[0].value[1] // "0"'; }
samples() { awk -F, -v m="$1" '$1 == m {print $3}' "$RUNNER_TEMP/k6.csv"; }

echo "tenant,tier,rate,achieved,p50_ms,p95_ms,p99_ms,errors_pct,throttled_pct,replicas" > "$out"
for tenant in "$@"; do
  tier=$(awk '/^tier:/ {print $2}' "tenants/$tenant/values.yaml")
  pods="namespace=\"tenant-$tenant\", container=\"workload\""
  for rate in ${RATES:-10 20 40 80 160}; do
    k6 run -q -e TENANT="$tenant" -e RATE="$rate" --out csv="$RUNNER_TEMP/k6.csv" tests/perf.js >/dev/null || true
    ratio=$(prom "sum(increase(container_cpu_cfs_throttled_periods_total{$pods}[1m])) / sum(increase(container_cpu_cfs_periods_total{$pods}[1m]))")
    replicas=$(prom "kube_deployment_status_replicas{namespace=\"tenant-$tenant\", deployment=\"workload\"}")
    latency=$(samples http_req_duration | sort -n | awk '{a[NR] = $1} END {printf "%d,%d,%d", a[int(NR * .5) + 1], a[int(NR * .95) + 1], a[int(NR * .99) + 1]}')
    requests=$(samples http_req_failed | awk '{n++; f += $1} END {printf "%d,%d", n / 60, n ? f * 100 / n : 0}')
    throttled=$(awk -v r="$ratio" 'BEGIN {printf "%d", r * 100}')
    echo "$tenant,$tier,$rate,${requests%,*},$latency,${requests#*,},$throttled,$replicas" | tee -a "$out"
    sleep 30
  done
done
