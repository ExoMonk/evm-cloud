output "installed" {
  description = "Whether the monitoring stack was installed."
  value       = var.enabled
}

output "grafana_service" {
  description = "Grafana service name for port-forwarding."
  value       = var.enabled ? "${var.project_name}-monitoring-grafana" : ""
}

output "grafana_url" {
  description = "Grafana URL (ingress hostname or port-forward command)."
  value = var.enabled ? (
    var.grafana_ingress_enabled
    ? "https://${var.grafana_hostname}"
    : "kubectl port-forward svc/${var.project_name}-monitoring-grafana 3000:80 -n ${var.namespace}"
  ) : ""
}
