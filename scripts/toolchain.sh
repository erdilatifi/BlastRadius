#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../versions.env"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

binary() {
  curl -fsSL "$2" -o "$tmp/$1"
  sudo install -m 0755 "$tmp/$1" "/usr/local/bin/$1"
}

archive() {
  curl -fsSL "$2" | tar -xz -C "$tmp" "$3"
  sudo install -m 0755 "$tmp/$3" "/usr/local/bin/$1"
}

zipped() {
  curl -fsSL "$2" -o "$tmp/$1.zip"
  unzip -q "$tmp/$1.zip" "$1" -d "$tmp"
  sudo install -m 0755 "$tmp/$1" "/usr/local/bin/$1"
}

install() {
  case $1 in
    rust) curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "$RUST_VERSION" -c clippy,rustfmt ;;
    node) curl -fsSL "https://nodejs.org/dist/v$NODE_VERSION/node-v$NODE_VERSION-linux-x64.tar.xz" \
      | sudo tar -xJ -C /usr/local --strip-components=1 --wildcards '*/bin' '*/include' '*/lib' '*/share' ;;
    terraform) zipped terraform "https://releases.hashicorp.com/terraform/$TERRAFORM_VERSION/terraform_${TERRAFORM_VERSION}_linux_amd64.zip" ;;
    tflint) zipped tflint "https://github.com/terraform-linters/tflint/releases/download/v$TFLINT_VERSION/tflint_linux_amd64.zip" ;;
    kubectl) binary kubectl "https://dl.k8s.io/release/v$KUBECTL_VERSION/bin/linux/amd64/kubectl" ;;
    k3d) binary k3d "https://github.com/k3d-io/k3d/releases/download/v$K3D_VERSION/k3d-linux-amd64" ;;
    cosign) binary cosign "https://github.com/sigstore/cosign/releases/download/v$COSIGN_VERSION/cosign-linux-amd64" ;;
    helm) archive helm "https://get.helm.sh/helm-v$HELM_VERSION-linux-amd64.tar.gz" linux-amd64/helm ;;
    kubeconform) archive kubeconform "https://github.com/yannh/kubeconform/releases/download/v$KUBECONFORM_VERSION/kubeconform-linux-amd64.tar.gz" kubeconform ;;
    kyverno) archive kyverno "https://github.com/kyverno/kyverno/releases/download/v$KYVERNO_VERSION/kyverno-cli_v${KYVERNO_VERSION}_linux_x86_64.tar.gz" kyverno ;;
    trivy) archive trivy "https://github.com/aquasecurity/trivy/releases/download/v$TRIVY_VERSION/trivy_${TRIVY_VERSION}_Linux-64bit.tar.gz" trivy ;;
    k6) archive k6 "https://github.com/grafana/k6/releases/download/v$K6_VERSION/k6-v$K6_VERSION-linux-amd64.tar.gz" "k6-v$K6_VERSION-linux-amd64/k6" ;;
    just) archive just "https://github.com/casey/just/releases/download/$JUST_VERSION/just-$JUST_VERSION-x86_64-unknown-linux-musl.tar.gz" just ;;
  esac
}

tools=("$@")
[ $# -gt 0 ] || tools=(rust node terraform tflint kubectl k3d cosign helm kubeconform kyverno trivy k6 just)
for tool in "${tools[@]}"; do
  install "$tool"
done
