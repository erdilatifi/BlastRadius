terraform {
  required_version = ">= 1.17"
}

variable "name" {
  type    = string
  default = "blastradius"
}

variable "servers" {
  type    = number
  default = 1
}

variable "agents" {
  type    = number
  default = 2
}

resource "terraform_data" "cluster" {
  triggers_replace = [var.name, var.servers, var.agents]

  input = {
    name       = var.name
    kubeconfig = abspath("${path.module}/kubeconfig")
    config = yamlencode({
      apiVersion = "k3d.io/v1alpha5"
      kind       = "Simple"
      metadata   = { name = var.name }
      servers    = var.servers
      agents     = var.agents
    })
  }

  provisioner "local-exec" {
    command = "config=$(mktemp --suffix .yaml) && printf '%s' \"$CONFIG\" > \"$config\" && k3d cluster create --config \"$config\" && k3d kubeconfig write \"$NAME\" --output \"$OUT\""
    environment = {
      CONFIG = self.input.config
      NAME   = self.input.name
      OUT    = self.input.kubeconfig
    }
  }

  provisioner "local-exec" {
    when    = destroy
    command = "k3d cluster delete \"$NAME\" && rm -f \"$OUT\""
    environment = {
      NAME = self.input.name
      OUT  = self.input.kubeconfig
    }
  }
}

output "kubeconfig" {
  value = terraform_data.cluster.output.kubeconfig
}
