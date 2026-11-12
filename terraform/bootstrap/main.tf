terraform {
  required_version = ">= 1.17"

  backend "kubernetes" {
    secret_suffix = "bootstrap"
    namespace     = "kube-system"
  }

  required_providers {
    helm = {
      source  = "hashicorp/helm"
      version = "3.3.0"
    }
  }
}

variable "kubeconfig" {
  type = string
}

variable "repo_url" {
  type    = string
  default = "https://github.com/erdilatifi/BlastRadius"
}

variable "revision" {
  type = string
}

provider "helm" {
  kubernetes = {
    config_path = var.kubeconfig
  }
}

resource "helm_release" "argocd" {
  name              = "argocd"
  namespace         = "argocd"
  create_namespace  = true
  chart             = "${path.module}/../../platform/argocd"
  dependency_update = true
}

resource "helm_release" "root" {
  name       = "root"
  namespace  = "argocd"
  repository = "https://argoproj.github.io/argo-helm"
  chart      = "argocd-apps"
  version    = "2.0.8"
  depends_on = [helm_release.argocd]

  values = [yamlencode({
    applications = {
      root = {
        namespace = "argocd"
        project   = "default"
        source = {
          repoURL        = var.repo_url
          targetRevision = var.revision
          path           = "platform/apps"
          helm           = { valuesObject = { repoURL = var.repo_url, revision = var.revision } }
        }
        destination = { server = "https://kubernetes.default.svc", namespace = "argocd" }
        syncPolicy  = { automated = { prune = true, selfHeal = true } }
      }
    }
  })]
}
