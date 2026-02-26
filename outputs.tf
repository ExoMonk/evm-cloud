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
