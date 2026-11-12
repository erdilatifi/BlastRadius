# Tenant crash looping

`TenantCrashLooping` (page): containers in `tenant-<name>` restarted more than three times in ten minutes. Requests still succeed while one replica is up, so this can fire before any burn alert.

## Check
1. `kubectl -n tenant-<name> get pods` and `kubectl -n tenant-<name> describe pod <pod>`: is the last state `OOMKilled`, `Error`, or a failed liveness probe?
2. Recent changes: the last commits under `tenants/<name>`. A new `image.tag` minutes before the restarts is the usual cause.
3. Logs of the previous container: `kubectl -n tenant-<name> logs <pod> --previous`.
4. The console's **Incidents** page: the Fix column shows what the remediation playbook proposed.

## Act
- A bad release: merge the `revert-release` PR, or revert the commit by hand; ArgoCD rolls back.
- `OOMKilled` three times in 30 minutes: review the `upgrade-tier` PR. Its body shows the capacity left after the move.
- Neither: leave the PR closed and follow the logs.

## Close
The alert resolves once restarts stop for ten minutes. Note the cause in the post-mortem draft.
