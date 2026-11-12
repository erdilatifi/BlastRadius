#!/usr/bin/env bash
set -uo pipefail

cd "$(dirname "$0")/.."
failed=0
check() {
  if [ "$2" = "$3" ]; then echo "ok    $1"; else echo "FAIL  $1: wanted $2, got $3"; failed=1; fi
}

kubectl -n blastradius-system rollout status deployment/controlplane --timeout=3m
kubectl -n kube-system port-forward svc/traefik 8083:80 >/dev/null &
trap 'kill $!' EXIT
sleep 3
console() { curl -s -H 'Host: blastradius.localhost' "$@"; }
auth=(-u "admin:$(cat terraform/cluster/console-password)")

check "no password gets 401" 401 "$(console -o /dev/null -w '%{http_code}' localhost:8083/api/tenants)"
check "console page is served" 200 "$(console "${auth[@]}" -o /dev/null -w '%{http_code}' localhost:8083/tenants)"
want=$(ls tenants | wc -l)
for _ in $(seq 36); do
  rows=$(console "${auth[@]}" localhost:8083/api/tenants)
  live=$(jq '[.[] | select(.availability != null and .p95Ms != null and .budgetRemaining != null and .health == "Healthy")] | length' <<<"$rows" 2>/dev/null)
  [ "${live:-0}" = "$want" ] && break
  sleep 5
done
jq -r '.[] | "\(.name) \(.tier) \(.health) availability=\(.availability) p95=\(.p95Ms)ms budget=\(.budgetRemaining)% burn=\(.burnRate)"' <<<"$rows"
check "every tenant has live SLIs" "$want" "${live:-0}"
console "${auth[@]}" localhost:8083/api/capacity | jq -c '{committed, fits}'

as=--as=system:serviceaccount:blastradius-system:controlplane
for verb in "create deployments" "patch deployments" "delete pods" "get secrets"; do
  check "control plane cannot $verb" no "$(kubectl auth can-i $verb -A $as)"
done
check "control plane can list pods" yes "$(kubectl auth can-i list pods -A $as)"
exit $failed
