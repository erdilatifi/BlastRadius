#!/usr/bin/env bash
set -uo pipefail

cd "$(dirname "$0")/.."
failed=0
check() {
  if [ "$2" = "$3" ]; then echo "ok    $1"; else echo "FAIL  $1: wanted $2, got $3"; failed=1; fi
}

kubectl -n argocd wait --for=jsonpath='{.status.health.status}'=Healthy application/chaos-mesh --timeout=5m
kubectl -n blastradius-system port-forward svc/controlplane 8080:8080 >/dev/null &
trap 'kill $!' EXIT
sleep 3
start() { curl -s -o /dev/null -w '%{http_code}' -H 'Content-Type: application/json' -d "$1" localhost:8080/api/runs; }

check "unknown experiment is refused" 404 "$(start '{"experiment":"rm-rf","tenant":"acme"}')"
check "platform namespace cannot be targeted" 403 "$(start '{"experiment":"pod-kill","tenant":"../kube-system"}')"
check "namespace without a tenant cannot be targeted" 403 "$(start '{"experiment":"pod-kill","tenant":"system"}')"
as=--as=system:serviceaccount:blastradius-system:controlplane
check "control plane can create podchaos" yes "$(kubectl auth can-i create podchaos.chaos-mesh.org -n tenant-acme $as)"
check "control plane still cannot delete deployments" no "$(kubectl auth can-i delete deployments -A $as)"

cargo run -q -p brctl -- chaos run pod-kill initech | tee /tmp/run.txt
check "pod-kill run finished with a score" 1 "$(grep -c '^blast radius' /tmp/run.txt)"
check "the platform namespace has no chaos objects" 0 "$(kubectl get podchaos -n blastradius-system -o name | wc -l)"
cargo run -q -p brctl -- ledger export --environment ci | jq -c '{experiment, target, phase, blast: .score.blastRadius, invalid: .score.invalid}'
exit $failed
