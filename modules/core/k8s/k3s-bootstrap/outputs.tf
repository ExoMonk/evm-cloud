output "kubeconfig_base64" {
  description = "Base64-encoded kubeconfig for the k3s cluster. Contains static admin credentials — use encrypted state backends."
  value       = fileexists(local.kubeconfig_file) ? trimspace(file(local.kubeconfig_file)) : ""
  sensitive   = true
  depends_on  = [terraform_data.fetch_secrets]
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
  value       = fileexists(local.token_file) ? trimspace(file(local.token_file)) : ""
  sensitive   = true
  depends_on  = [terraform_data.fetch_secrets]
}
