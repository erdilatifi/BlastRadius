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

echo "tenant,tier,rate,achieved,p50_ms,p95_ms,p99_ms,errors,throttled,replicas" > "$out"
for tenant in "$@"; do
  tier=$(awk '/^tier:/ {print $2}' "tenants/$tenant/values.yaml")
  pods="namespace=\"tenant-$tenant\", container=\"workload\""
  for rate in ${RATES:-10 20 40 80 160}; do
    k6 run -q -e TENANT="$tenant" -e RATE="$rate" -e SUMMARY="$RUNNER_TEMP/k6.json" tests/perf.js >/dev/null || true
    throttled=$(prom "sum(increase(container_cpu_cfs_throttled_periods_total{$pods}[1m])) / sum(increase(container_cpu_cfs_periods_total{$pods}[1m]))")
    replicas=$(prom "kube_deployment_status_replicas{namespace=\"tenant-$tenant\", deployment=\"workload\"}")
    jq -r --arg row "$tenant,$tier,$rate" --arg throttled "$throttled" --arg replicas "$replicas" '.metrics as $m
      | [$row, ($m.iterations.values.rate | floor), ($m.http_req_duration.values | .["p(50)"], .["p(95)"], .["p(99)"] | floor),
         ($m.http_req_failed.values.rate * 100 | floor), ($throttled | tonumber * 100 | floor), $replicas] | join(",")' \
      "$RUNNER_TEMP/k6.json" | tee -a "$out"
    sleep 30
  done
done
