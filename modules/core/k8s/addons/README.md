# K8s Addons

Third-party Helm charts managed by Terraform. Used for monitoring, ingress, streaming, and other infrastructure services that ship as community Helm charts.

## Provider Inheritance

This module must be called from a root that configures both `kubernetes` and `helm` providers. The caller **must** pass providers explicitly:

```hcl
module "k8s_addons" {
  source = "../../core/k8s/addons"
  providers = {
    kubernetes = kubernetes
    helm       = helm
  }
  project_name = var.project_name
}
```

Never configure providers inside this module or its sub-modules.

## Adding a New Addon

1. Create a sub-directory: `addons/<name>/`
2. Add `main.tf` with `helm_release` resource, `variables.tf`, `outputs.tf`
3. Add an enable flag in the parent `variables.tf` (e.g., `var.monitoring_enabled`)
4. Add a conditional `module` call in the parent `main.tf`:

```hcl
module "monitoring" {
  source       = "./monitoring"
  count        = var.monitoring_enabled ? 1 : 0
  project_name = var.project_name
  namespace    = var.namespace
}
```

## Chart Version Pinning

Every Helm chart version is a variable with a pinned default:

```hcl
variable "kube_prometheus_stack_version" {
  type    = string
  default = "65.1.0"
}
```

Bump via variable override, not code change. This makes upgrades explicit and auditable.

## Naming Convention

All Helm release names follow: `${var.project_name}-<addon-name>`

Example: `my-project-monitoring`, `my-project-ingress`

## Namespace Convention

Each addon may override the namespace or use the shared `var.namespace` (default: `addons`). Addons that create their own namespace should set `create_namespace = true` in the `helm_release`.

## Migration Note

Existing users adding this module for the first time need to fetch the new Helm provider:

```bash
terraform init -upgrade=hashicorp/helm
```

Using the scoped `-upgrade=hashicorp/helm` flag avoids upgrading other providers (AWS, kubernetes) in the lock file.
