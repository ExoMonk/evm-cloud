output "service_name" {
  description = "Kubernetes Deployment name for the rindexer indexer."
  value       = kubernetes_deployment.indexer.metadata[0].name
}

output "log_group_name" {
  description = "Empty for EKS — logs available via kubectl logs."
  value       = ""
}
