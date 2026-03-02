output "service_name" {
  description = "Kubernetes Service name for the eRPC proxy."
  value       = kubernetes_service.erpc.metadata[0].name
}

output "container_port" {
  description = "Port the eRPC proxy listens on."
  value       = var.container_port
}
