#!/usr/bin/env bash
set -uo pipefail

a=$1 b=$2
cd "$(dirname "$0")/.."
image=ghcr.io/erdilatifi/blastradius/workload:$(sed -n 's/^  tag: //p' "tenants/$a/values.yaml")
failed=0

check() {
  if [ "$2" = "$3" ]; then echo "ok    $1"; else echo "FAIL  $1: wanted $2, got $3"; failed=1; fi
}

dial() {
  local ns=$1 target=$2 pod=dial-$RANDOM phase=
  kubectl -n "$ns" apply -f - >/dev/null <<EOF
apiVersion: v1
kind: Pod
metadata:
  name: $pod
  labels:
    tenant: ${ns#tenant-}
spec:
  restartPolicy: Never
  automountServiceAccountToken: false
  securityContext:
    runAsNonRoot: true
    seccompProfile:
      type: RuntimeDefault
  containers:
    - name: dial
      image: $image
      args: [dial, $target]
      resources:
        requests: {cpu: 10m, memory: 16Mi}
        limits: {cpu: 50m, memory: 32Mi}
      securityContext:
        allowPrivilegeEscalation: false
        readOnlyRootFilesystem: true
        capabilities:
          drop: [ALL]
EOF
  for _ in $(seq 60); do
    phase=$(kubectl -n "$ns" get pod "$pod" -o jsonpath='{.status.phase}')
    case $phase in Succeeded | Failed) break ;; esac
    sleep 2
  done
  kubectl -n "$ns" delete pod "$pod" --wait=false >/dev/null
  [ "$phase" = Succeeded ] && echo open || echo closed
}

can() {
  kubectl auth can-i "$1" "$2" -n "tenant-$3" --as dev --as-group "tenant-$a-devs" 2>/dev/null || true
}

check "tenant-$a reaches cluster DNS" open "$(dial "tenant-$a" kube-dns.kube-system:53)"
check "tenant-$a cannot reach tenant-$b public port" closed "$(dial "tenant-$a" "workload.tenant-$b:8080")"
check "tenant-$a cannot reach tenant-$b admin port" closed "$(dial "tenant-$a" "workload.tenant-$b:9090")"
check "default namespace reaches cluster DNS" open "$(dial default kube-dns.kube-system:53)"
check "default namespace cannot reach tenant-$b" closed "$(dial default "workload.tenant-$b:8080")"

kubectl -n kube-system port-forward svc/traefik 8081:80 >/dev/null &
forward=$!
sleep 3
code=$(curl -s -o /dev/null -w '%{http_code}' -H "Host: $a.blastradius.localhost" localhost:8081/healthz)
kill $forward
check "ingress reaches tenant-$a" 200 "$code"

check "tenant-$a devs read their pods" yes "$(can list pods "$a")"
check "tenant-$a devs cannot change their deployments" no "$(can patch deployments "$a")"
check "tenant-$a devs cannot read tenant-$b" no "$(can list pods "$b")"

for f in tests/admission/*.yaml; do
  case $f in */kyverno-test.yaml | */values.yaml) continue ;; esac
  if out=$(kubectl apply --dry-run=server -f "$f" 2>&1); then result=admitted; else result=rejected; fi
  want=rejected
  [ "$(basename "$f")" = good.yaml ] && want=admitted
  check "$(basename "$f" .yaml) is $want" "$want" "$result"
  [ "$result" = rejected ] && echo "      $out"
done

exit $failed
