terraform {
  required_version = ">= 1.14.6"
}

module "evm_cloud" {
  source = "../.."

  project_name            = var.project_name
  infrastructure_provider = var.infrastructure_provider
  deployment_target       = var.deployment_target
  runtime_arch            = var.runtime_arch
  database_mode           = var.database_mode
  streaming_mode          = var.streaming_mode
  ingress_mode            = var.ingress_mode
}

output "provider_selection" {
  value = module.evm_cloud.provider_selection
}

output "capability_contract" {
  value = module.evm_cloud.capability_contract
}
