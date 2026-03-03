# K8s Addons — Third-party Helm charts managed by Terraform.
#
# Each addon is a sub-module in its own directory (e.g., ./monitoring/, ./ingress/).
# Sub-modules are called conditionally based on enable flags.
# DO NOT add helm_release resources directly in this file — always use sub-modules.
#
# This module inherits both `kubernetes` and `helm` providers from its caller.
# It must be called with explicit `providers = { kubernetes = kubernetes, helm = helm }`.

module "eso" {
  source = "./eso"

  enabled           = var.eso_enabled
  eso_chart_version = var.eso_chart_version
}

