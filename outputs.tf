locals {
  provider_outputs = {
    aws        = var.infrastructure_provider == "aws" ? module.provider_aws[0] : null
    bare_metal = var.infrastructure_provider == "bare_metal" ? module.provider_bare_metal[0] : null
  }
}

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

output "adapter_context" {
  description = "Active provider adapter context."
  value       = try(local.provider_outputs[var.infrastructure_provider].adapter_context, null)
}

output "networking" {
  description = "Networking outputs; null for bare_metal (user-managed)."
  value       = try(local.provider_outputs[var.infrastructure_provider].networking, null)
}

output "postgres" {
  description = "PostgreSQL outputs; null unless postgres_enabled and provider=aws."
  value       = try(local.provider_outputs[var.infrastructure_provider].postgres, null)
}

output "rpc_proxy" {
  description = "eRPC proxy outputs from active provider."
  value       = try(local.provider_outputs[var.infrastructure_provider].rpc_proxy, null)
}

output "indexer" {
  description = "rindexer outputs from active provider."
  value       = try(local.provider_outputs[var.infrastructure_provider].indexer, null)
}

output "workload_handoff" {
  description = "Handoff contract for external deployers."
  value       = try(local.provider_outputs[var.infrastructure_provider].workload_handoff, null)
}
