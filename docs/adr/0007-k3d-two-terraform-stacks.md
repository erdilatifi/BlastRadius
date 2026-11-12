# ADR-0007: k3d, two Terraform stacks, and where state lives

**Status:** accepted

## Decision

- The cluster is k3d (k3s in Docker), created by `terraform/cluster` with a `terraform_data` resource that renders a k3d config and runs the `k3d` CLI.
- Terraform is split into two stacks:

| Stack | Creates | State |
|---|---|---|
| `terraform/cluster` | The k3d cluster. Inputs: name, server and agent counts. Output: kubeconfig path | Local file, gitignored |
| `terraform/bootstrap` | ArgoCD (Helm provider) and the root Application pointing at `platform/apps` | Kubernetes backend: a Secret in `kube-system`, locked with a Lease |

- `terraform/envs/<env>.tfvars` holds what differs between a Codespace and a CI runner. `just up <env>` applies both stacks and waits until ArgoCD is synced; `just down` destroys the cluster stack.
- ArgoCD is installed from `platform/argocd`, a wrapper chart around `argo-cd`. Terraform installs it once; from then on the `argocd` Application in `platform/apps` manages the same chart from Git, so the version and values live in one place.
- tflint runs in CI next to `terraform validate`, catching unused declarations and missing version constraints.

## Why

- k3d fits Codespaces and Actions runners: one Docker daemon, a multi-node cluster in under a minute, with ingress, storage and NetworkPolicy built in.
- Every community k3d Terraform provider is unmaintained, so the CLI behind `terraform_data` is the smaller and safer dependency.
- The cluster stack cannot keep state in the cluster it creates, and a remote bucket would cost money and credentials. Its state describes one throwaway local cluster, so a local file is enough.
- The bootstrap stack's state lives next to what it describes. Lease locking stops two applies from racing, and deleting the cluster deletes the state with it, so `just down` leaves nothing behind.
- The split is the interface: an EKS or GKE module that takes the same inputs and returns a kubeconfig would replace `terraform/cluster`, and `terraform/bootstrap` would not change.

## Alternatives rejected

- **kind or minikube.** kind has no bundled ingress or load balancer; minikube in Docker supports only one node well.
- **One stack.** The Helm provider would need a kubeconfig that does not exist until apply, and the cluster's own state would have to live inside it.
- **Remote state (S3, Terraform Cloud).** Costs money or an account and credentials for a cluster that lives a few hours.
