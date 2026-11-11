doctor:
    #!/usr/bin/env bash
    status=0
    for tool in git docker python3 cargo rustc node npm kubectl k3d terraform helm kyverno trivy cosign k6 just; do
      case $tool in
        kubectl) args="version --client" ;;
        k3d|kyverno|cosign|k6|terraform) args="version" ;;
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
