# Platform capacity near the gate

`PlatformCapacityNearGate` (ticket): the ResourceQuotas of all tenants together commit more than 75% of the CPU, memory or pods declared in `platform/capacity.yaml`. At 85% the capacity gate refuses new tenants and tier upgrades, in `brctl` and in CI.

The alert counts commitments, not use. A tenant that idles still holds its whole quota.

## Check
1. `brctl tenant list`: the last line shows committed against capacity for each resource.
2. The latest weekly report in `docs/reports/`: the forecast says how fast commitments have grown, and the rightsizing list names tenants that hold far more than they use.

## Act
- A tenant on the rightsizing list: open a PR moving it to the suggested tier, after asking its owner.
- Real growth: add capacity, then raise the numbers in `platform/capacity.yaml` in the same PR that adds the node.
- Neither: hold new onboarding until one of the two is possible.

## Close
The alert resolves when commitments fall back under 75%, or when capacity is raised.
