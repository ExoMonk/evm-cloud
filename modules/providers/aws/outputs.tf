output "adapter_context" {
  description = "Resolved AWS adapter context from provider-neutral root inputs."
  value       = local.adapter_context
}

output "networking" {
  description = "Networking outputs from AWS adapter, or null when disabled."
  value       = local.networking
}

output "postgres" {
  description = "PostgreSQL outputs; null when disabled."
  value = var.postgres_enabled ? {
    endpoint          = module.postgres[0].endpoint
    port              = module.postgres[0].port
    db_name           = module.postgres[0].db_name
    master_secret_arn = module.postgres[0].master_secret_arn
  } : null
}

output "eks_cluster_name" {
  description = "EKS cluster name; empty when compute_engine != eks."
  value       = local.any_eks_compute ? module.eks_cluster[0].cluster_name : ""
}

output "rpc_proxy" {
  description = "eRPC proxy outputs; null when disabled."
  value = var.rpc_proxy_enabled ? {
    service_name   = var.compute_engine == "ecs" ? module.rpc_proxy[0].service_name : module.eks_rpc_proxy[0].service_name
    container_port = var.compute_engine == "ecs" ? module.rpc_proxy[0].container_port : module.eks_rpc_proxy[0].container_port
  } : null
}

output "indexer" {
  description = "rindexer indexer outputs; null when disabled."
  value = var.indexer_enabled ? {
    service_name   = var.compute_engine == "ecs" ? module.indexer[0].service_name : module.eks_indexer[0].service_name
    log_group_name = var.compute_engine == "ecs" ? module.indexer[0].log_group_name : module.eks_indexer[0].log_group_name
  } : null
}
