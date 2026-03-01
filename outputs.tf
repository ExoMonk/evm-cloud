output "provider_selection" {
  description = "Active infrastructure provider selection."
  value = {
    infrastructure_provider = var.infrastructure_provider
    deployment_target       = var.deployment_target
    runtime_arch            = var.runtime_arch
  }
}

output "capability_contract" {
  description = "Provider-neutral capability contract used by adapters."
  value       = module.capabilities.contract
}

output "aws_adapter_context" {
  description = "AWS adapter context; null unless infrastructure_provider=aws."
  value       = var.infrastructure_provider == "aws" ? module.provider_aws[0].adapter_context : null
}

output "networking" {
  description = "Networking outputs from AWS adapter; null unless networking_enabled and provider=aws."
  value       = var.infrastructure_provider == "aws" ? module.provider_aws[0].networking : null
}

output "postgres" {
  description = "PostgreSQL outputs; null unless postgres_enabled and provider=aws."
  value       = var.infrastructure_provider == "aws" ? module.provider_aws[0].postgres : null
}

output "rpc_proxy" {
  description = "eRPC proxy outputs; null unless rpc_proxy_enabled and provider=aws."
  value       = var.infrastructure_provider == "aws" ? module.provider_aws[0].rpc_proxy : null
}

output "indexer" {
  description = "rindexer outputs; null unless indexer_enabled and provider=aws."
  value       = var.infrastructure_provider == "aws" ? module.provider_aws[0].indexer : null
}

output "workload_handoff" {
  description = "Handoff contract for external deployers; null unless provider=aws."
  value       = var.infrastructure_provider == "aws" ? module.provider_aws[0].workload_handoff : null
}
