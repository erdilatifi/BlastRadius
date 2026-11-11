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

curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "$RUST_VERSION" -c clippy,rustfmt

curl -fsSL "https://nodejs.org/dist/v$NODE_VERSION/node-v$NODE_VERSION-linux-x64.tar.xz" \
  | sudo tar -xJ -C /usr/local --strip-components=1 --wildcards '*/bin' '*/include' '*/lib' '*/share'

curl -fsSL "https://releases.hashicorp.com/terraform/$TERRAFORM_VERSION/terraform_${TERRAFORM_VERSION}_linux_amd64.zip" -o "$tmp/terraform.zip"
unzip -q "$tmp/terraform.zip" terraform -d "$tmp"
sudo install -m 0755 "$tmp/terraform" /usr/local/bin/terraform

binary kubectl "https://dl.k8s.io/release/v$KUBECTL_VERSION/bin/linux/amd64/kubectl"
binary k3d "https://github.com/k3d-io/k3d/releases/download/v$K3D_VERSION/k3d-linux-amd64"
binary cosign "https://github.com/sigstore/cosign/releases/download/v$COSIGN_VERSION/cosign-linux-amd64"
archive helm "https://get.helm.sh/helm-v$HELM_VERSION-linux-amd64.tar.gz" linux-amd64/helm
archive kyverno "https://github.com/kyverno/kyverno/releases/download/v$KYVERNO_VERSION/kyverno-cli_v${KYVERNO_VERSION}_linux_x86_64.tar.gz" kyverno
archive trivy "https://github.com/aquasecurity/trivy/releases/download/v$TRIVY_VERSION/trivy_${TRIVY_VERSION}_Linux-64bit.tar.gz" trivy
archive k6 "https://github.com/grafana/k6/releases/download/v$K6_VERSION/k6-v$K6_VERSION-linux-amd64.tar.gz" "k6-v$K6_VERSION-linux-amd64/k6"
archive just "https://github.com/casey/just/releases/download/$JUST_VERSION/just-$JUST_VERSION-x86_64-unknown-linux-musl.tar.gz" just
