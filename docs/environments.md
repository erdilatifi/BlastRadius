# Environments

| | GitHub Actions runner | GitHub Codespace |
|---|---|---|
| Machine | 4 vCPU, 16 GB, `ubuntu-24.04`, kernel 7.0 (Azure) | 4 cores, 16 GB, Docker-in-Docker, kernel 6.8 (Azure) |
| Bring-up | `just up ci` | `just up codespace` |
| Agents | 1 | 2 |
| Revision ArgoCD deploys | The commit under test | `main` |

## Chaos Mesh spike

Chaos Mesh 2.8.8 on k3d 5.9.0 (k3s v1.35.5), daemon with `runtime=containerd` and `socketPath=/run/k3s/containerd/containerd.sock`. Faults aimed at a `busybox` pod.

| Fault | Actions runner (2026-11-12) | Codespace (2026-11-12) |
|---|---|---|
| PodChaos `container-kill` | Works: container restarted within seconds | Works: restart count went to 1 |
| StressChaos CPU + memory | Works: `stress-ng` and `memStress` ran inside the pod's PID namespace | Works: `AllInjected` reported. The stressors were not visible from the pod's own `ps` |
| NetworkChaos `delay` 200 ms | Works: pod-to-pod ping went from 0.2 ms to 200.2 ms. `sch_netem` ships with the runner kernel | Works: ping went from 0.19 ms to 200.2 ms |

The daemon logs `fail to find device cgroup` for `/dev/fuse` at start. That only affects IOChaos, which this project does not use.

Decision: `network-delay` uses the kernel (NetworkChaos), not the app-level latency fault. It works in both environments.
