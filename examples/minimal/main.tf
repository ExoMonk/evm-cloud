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

  aws_region                      = var.aws_region
  aws_skip_credentials_validation = var.aws_skip_credentials_validation
  networking_enabled              = var.networking_enabled
  network_environment             = var.network_environment
  network_vpc_cidr                = var.network_vpc_cidr
  network_availability_zones      = var.network_availability_zones
  network_enable_nat_gateway      = var.network_enable_nat_gateway
  network_enable_vpc_endpoints    = var.network_enable_vpc_endpoints
}

output "provider_selection" {
  value = module.evm_cloud.provider_selection
}

output "capability_contract" {
  value = module.evm_cloud.capability_contract
}
