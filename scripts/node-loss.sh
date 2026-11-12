#!/usr/bin/env bash
set -euo pipefail

node=${NODE:-k3d-blastradius-agent-0}
window=${WINDOW:-480}
cd "$(dirname "$0")/.."
ip=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' k3d-blastradius-server-0)
tenants=$(ls tenants)
kubectl get pods -A -l tenant -o wide
kubectl get pods -A -l app.kubernetes.io/name=traefik -o wide
docker stop "$node" >/dev/null
start=$(date +%s)
declare -A failed last
while [ $(($(date +%s) - start)) -lt "$window" ]; do
  for t in $tenants; do
    code=$(curl -s -o /dev/null -w '%{http_code}' -m 2 -H "Host: $t.blastradius.localhost" "http://$ip/api/quote?items=1" || true)
    if [ "$code" != 200 ]; then
      failed[$t]=$((${failed[$t]:-0} + 1))
      last[$t]=$(($(date +%s) - start))
    fi
  done
  sleep 1
done
echo "tenant,failed_probes,recovered_after_s"
for t in $tenants; do echo "$t,${failed[$t]:-0},${last[$t]:-0}"; done
kubectl get pods -A -l tenant -o wide
docker start "$node" >/dev/null
