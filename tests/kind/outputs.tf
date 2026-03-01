output "rpc_proxy_service_name" {
  description = "eRPC service name created in kind."
  value       = module.rpc_proxy.service_name
}

output "indexer_service_name" {
  description = "Indexer deployment name created in kind."
  value       = module.indexer.service_name
}
