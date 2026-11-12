doctor:
    #!/usr/bin/env bash
    status=0
    for tool in git docker python3 cargo rustc node npm kubectl k3d terraform tflint helm kyverno trivy cosign k6 just; do
      case $tool in
        kubectl) args="version --client" ;;
        k3d|kyverno|cosign|k6|terraform|tflint) args="version" ;;
        helm) args="version --short" ;;
        *) args="--version" ;;
      esac
      if command -v "$tool" >/dev/null; then
        printf '%-10s %s\n' "$tool" "$($tool $args 2>&1 | grep -m1 -oE '[0-9]+\.[0-9]+(\.[0-9]+)?')"
      else
        printf '%-10s MISSING\n' "$tool"
        status=1
      fi
    done
    exit $status

check: rust web reports

rust:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test

[working-directory: 'web']
web:
    npm ci
    npm run lint
    npm test
    npm run build

[working-directory: 'reports']
reports:
    ruff check
    ruff format --check
    pytest

up env:
    #!/usr/bin/env bash
    set -euo pipefail
    terraform -chdir=terraform/cluster init -input=false
    terraform -chdir=terraform/cluster apply -input=false -auto-approve -var-file=../envs/{{env}}.tfvars
    kubeconfig=$(terraform -chdir=terraform/cluster output -raw kubeconfig)
    terraform -chdir=terraform/bootstrap init -input=false -reconfigure -backend-config="config_path=$kubeconfig"
    terraform -chdir=terraform/bootstrap apply -input=false -auto-approve -var-file=../envs/{{env}}.tfvars -var="kubeconfig=$kubeconfig"
    export KUBECONFIG=$kubeconfig
    kubectl -n argocd wait --for=create application/argocd --timeout=5m
    kubectl -n argocd wait --for=jsonpath='{.status.sync.status}'=Synced application/root application/argocd --timeout=10m
    kubectl -n argocd wait --for=jsonpath='{.status.health.status}'=Healthy application/root application/argocd --timeout=10m

down:
    terraform -chdir=terraform/cluster destroy -input=false -auto-approve
