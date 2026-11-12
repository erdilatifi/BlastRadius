#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
rendered=$(for values in tenants/*/values.yaml; do helm template x charts/tenant -f "$values"; done)
alerts=$(grep -c '^ *- alert:' <<<"$rendered")
urls=$(grep -o 'runbook_url: .*' <<<"$rendered" | sed 's|.*/blob/main/||')
[ "$alerts" -eq "$(wc -l <<<"$urls")" ] || { echo "an alert has no runbook_url"; exit 1; }
for file in $(sort -u <<<"$urls"); do
  [ -s "$file" ] || { echo "missing $file"; exit 1; }
done
echo "$alerts alerts, every runbook exists"
