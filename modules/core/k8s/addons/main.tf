# K8s Addons — Third-party Helm charts managed by Terraform.
#
# Each addon is a sub-module in its own directory (e.g., ./monitoring/, ./ingress/).
# Sub-modules are called conditionally based on enable flags.
# DO NOT add helm_release resources directly in this file — always use sub-modules.
#
# Example (future):
#   module "monitoring" {
#     source = "./monitoring"
#     count  = var.monitoring_enabled ? 1 : 0
#     project_name = var.project_name
#     namespace    = var.namespace
#   }
#
# This module inherits both `kubernetes` and `helm` providers from its caller.
# It must be called with explicit `providers = { kubernetes = kubernetes, helm = helm }`.

