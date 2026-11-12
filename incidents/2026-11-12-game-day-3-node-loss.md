# Game day 3: node loss

**Staged on purpose.** `scripts/node-loss.sh` stops the agent node and probes every tenant in a loop for eight minutes. 2026-11-12.

## Summary

Stopping one of two nodes took every tenant offline for six minutes, even though each tenant had a healthy replica on the surviving node. The single Traefik pod was on the node that stopped. With two Traefik replicas on different nodes, the same failure costs about 45 seconds.

## Impact

| Run | Traefik | acme | globex | initech |
|---|---|---|---|---|
| [43190016695](https://github.com/erdilatifi/BlastRadius/actions/runs/43190016695), before | 1 replica | 114 failed probes, 360 s | 114, 361 s | 113, 359 s |
| [43252379452](https://github.com/erdilatifi/BlastRadius/actions/runs/43252379452), after | 2 replicas | 7 failed probes, 49 s | 7, 44 s | 6, 46 s |
| [43263251495](https://github.com/erdilatifi/BlastRadius/actions/runs/43263251495), after | 2 replicas | 6 failed probes, 41 s | 7, 46 s | 6, 43 s |

The time is from the node stopping to the last failed probe. All three tenants failed together in every run.

## Detection

The probe loop in the script, not an alert. The node is stopped after the campaign has exported its incidents, so this exercise does not measure how fast the burn-rate alerts would have fired.

## Timeline

| Time after the stop | Before, 1 replica | After, 2 replicas |
|---|---|---|
| 0 s | The only Traefik pod is on the stopped node; every probe fails | One of two Traefik pods and one replica of each tenant are on the stopped node; some probes fail |
| 41 to 49 s | Still failing | Last failed probe |
| 359 to 361 s | Last failed probe | Serving |
| 480 s | Serving | Serving; the tenants' lost replicas have been rescheduled on the server node |

## Root cause

In the before run every tenant happened to have a replica on each node, but the ingress they all share did not. Traefik ran as one pod, so losing its node was a full outage for everyone. Kubernetes gives a pod on an unreachable node 300 seconds by default before it evicts and reschedules it, which, added to the time it takes to mark the node NotReady, accounts for the 360 seconds.

## Fix

`0318242`: two Traefik replicas with a topology spread constraint on `kubernetes.io/hostname`, set through the k3s `HelmChartConfig`.

## Prevention

- Traefik keeps one replica per node, and `94c967c` makes the same spread a rule for tenant replicas instead of luck. A single node loss leaves a working path.
- The remaining 45 seconds is most likely failure detection: the time until the node is marked NotReady and its pods leave the Services. Shortening it means tuning the node heartbeat or adding retries at the client, and neither was done here.

## Found along the way

The first "after" run and the CI run on the same commit had every chaos experiment refused with `x509: certificate signed by unknown authority` from the Chaos Mesh webhook. A debug run showed ArgoCD re-rendering the Chaos Mesh chart six times while the platform came up, and the chart generates new certificates on every render. `1d5fc19` makes ArgoCD keep the certificates it first installed. The three failed runs stay in the ledger. Three runs since then passed, which is evidence, not proof, for a fault that depends on timing.

## Runbook

[Tenant availability burn](../runbooks/tenant-availability-burn.md), step 2: several tenants failing at once points at the platform, Traefik or a node, not at a tenant.

## Lessons

- Spreading the tenants protected nothing while the thing in front of them was a single pod. The shared parts of a platform set its real blast radius.
- A 300-second default is a long time. It only showed because the experiment measured recovery instead of assuming it.
- The run that verified this fix exposed an unrelated fault in the chaos tooling. The campaign and CI caught it on the first run, which is what they are for.
