output "release_prefix" {
  description = "Helm release name prefix derived from project_name. Used by addon sub-modules."
  value       = var.project_name
}

output "namespace" {
  description = "Default namespace for addon deployments."
  value       = var.namespace
}

output "monitoring_installed" {
  description = "Whether the monitoring stack was installed."
  value       = module.monitoring.installed
}

output "grafana_service" {
  description = "Grafana service name for port-forwarding."
  value       = module.monitoring.grafana_service
}

output "grafana_url" {
  description = "Grafana URL (ingress hostname or port-forward command)."
  value       = module.monitoring.grafana_url
}
