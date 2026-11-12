#!/usr/bin/env bash
set -euo pipefail

bad=$1 good=$2
cd "$(dirname "$0")/.."
pids=()
trap 'kill "${pids[@]}" 2>/dev/null || true' EXIT

forward() {
  kubectl port-forward "$@" >/dev/null &
  pids+=($!)
}

query() {
  curl -sf localhost:9091/api/v1/query --data-urlencode "query=$1" | jq -r '.data.result[] | "\(.metric.tenant // "-") \(.metric.slo // "-") \(.value[1])"' || true
}

port=19090
for pod in $(kubectl -n "tenant-$bad" get pods -l app.kubernetes.io/name=workload -o name); do
  forward -n "tenant-$bad" "$pod" "$port:9090"
  sleep 2
  curl -sf -X PUT -H 'content-type: application/json' -d '{"error_rate":0.5,"latency_ms":0,"leak_kib":0}' "localhost:$port/admin/fault"
  echo " set on $pod"
  port=$((port + 1))
done

forward -n kube-system svc/traefik 8082:80
forward -n monitoring svc/prometheus-operated 9091:9090
sleep 2

end=$((SECONDS + 600)) i=0
while [ $SECONDS -lt $end ]; do
  i=$((i + 1))
  for t in "$bad" "$good"; do
    for _ in 1 2 3 4 5; do curl -s -o /dev/null -H "Host: $t.blastradius.localhost" localhost:8082/api/quote; done
  done
  if [ $((i % 20)) -eq 0 ]; then
    firing=$(query "ALERTS{alertname=\"TenantAvailabilityFastBurn\",alertstate=\"firing\"}")
    if grep -q "^$bad " <<<"$firing"; then break; fi
  fi
  sleep 0.5
done

echo "burn rate over 1h:"
query 'tenant:slo_errors:ratio_rate1h / tenant:slo_budget:ratio'
firing=$(query 'ALERTS{alertstate="firing",tenant!=""}')
echo "firing tenant alerts:"
echo "$firing"
grep -q "^$bad availability" <<<"$firing" || { echo "FAIL  no fast burn alert for $bad"; exit 1; }
if grep -q "^$good " <<<"$firing"; then echo "FAIL  $good alerted"; exit 1; fi
url=$(curl -sf localhost:9091/api/v1/alerts | jq -r --arg t "$bad" '.data.alerts[] | select(.labels.tenant == $t and .labels.alertname == "TenantAvailabilityFastBurn") | .annotations.runbook_url' | head -1)
test -s "runbooks/${url##*/}" || { echo "FAIL  runbook $url missing"; exit 1; }
echo "ok    $bad fast burn fired after ${SECONDS}s with runbook ${url##*/}, $good flat"
