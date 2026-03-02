output "release_prefix" {
  description = "Helm release name prefix derived from project_name. Used by addon sub-modules."
  value       = var.project_name
}

output "namespace" {
  description = "Default namespace for addon deployments."
  value       = var.namespace
}
