#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
platform=(-f charts/tenant/values.yaml -f platform/capacity.yaml -f platform/chaos/catalog.yaml -f charts/blastradius/values.yaml)
rendered=$(for values in tenants/*/values.yaml; do helm template x charts/tenant -f "$values"; done; helm template x charts/blastradius "${platform[@]}")
alerts=$(grep -c '^ *- alert:' <<<"$rendered")
urls=$(grep -o 'runbook_url: .*' <<<"$rendered" | sed 's|.*/blob/main/||')
[ "$alerts" -eq "$(wc -l <<<"$urls")" ] || { echo "an alert has no runbook_url"; exit 1; }
for file in $(sort -u <<<"$urls"); do
  [ -s "$file" ] || { echo "missing $file"; exit 1; }
done
echo "$alerts alerts, every runbook exists"
