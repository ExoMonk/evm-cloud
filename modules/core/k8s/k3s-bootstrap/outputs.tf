output "kubeconfig_base64" {
  description = "Base64-encoded kubeconfig for the k3s cluster. Contains static admin credentials — use encrypted state backends."
  value       = data.external.kubeconfig.result.value
  sensitive   = true
}

output "cluster_endpoint" {
  description = "k3s API server endpoint."
  value       = "https://${var.host_address}:6443"
}

output "node_name" {
  description = "k3s server node name."
  value       = local.node_name
}

output "node_token" {
  description = "k3s server node token for worker nodes to join the cluster. Sensitive — only stored in Terraform state."
  value       = data.external.node_token.result.value
  sensitive   = true
}
