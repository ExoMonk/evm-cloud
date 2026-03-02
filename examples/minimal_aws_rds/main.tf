terraform {
  required_version = ">= 1.14.6"
}

module "evm_cloud" {
  source = "../.."

  project_name                       = var.project_name
  infrastructure_provider            = var.infrastructure_provider
  deployment_target                  = var.deployment_target
  runtime_arch                       = var.runtime_arch
  database_mode                      = var.database_mode
  streaming_mode                     = var.streaming_mode
  ingress_mode                       = var.ingress_mode
  compute_engine                     = var.compute_engine
  workload_mode                      = var.workload_mode
  ssh_public_key                     = var.ssh_public_key
  ec2_instance_type                  = var.ec2_instance_type
  ec2_secret_recovery_window_in_days = 0 # Dev: immediate deletion for easy re-apply

  aws_region                      = var.aws_region
  aws_skip_credentials_validation = var.aws_skip_credentials_validation
  networking_enabled              = var.networking_enabled
  network_environment             = var.network_environment
  network_vpc_cidr                = var.network_vpc_cidr
  network_availability_zones      = var.network_availability_zones
  network_enable_nat_gateway      = var.network_enable_nat_gateway
  network_enable_vpc_endpoints    = var.network_enable_vpc_endpoints

  # Postgres
  postgres_enabled          = var.postgres_enabled
  postgres_instance_class   = var.postgres_instance_class
  postgres_engine_version   = var.postgres_engine_version
  postgres_db_name          = var.postgres_db_name
  postgres_db_username      = var.postgres_db_username
  postgres_backup_retention = var.postgres_backup_retention

  # RPC Proxy
  rpc_proxy_enabled = var.rpc_proxy_enabled
  rpc_proxy_image   = var.rpc_proxy_image

  # Indexer
  indexer_enabled         = var.indexer_enabled
  indexer_image           = var.indexer_image
  indexer_rpc_url         = var.indexer_rpc_url
  indexer_storage_backend = var.indexer_storage_backend

  # Config injection — read from files alongside this example
  erpc_config_yaml     = file("${path.module}/config/erpc.yaml")
  rindexer_config_yaml = file("${path.module}/config/rindexer.yaml")
  rindexer_abis = {
    "ERC20.json" = file("${path.module}/config/abis/ERC20.json")
  }
}

output "provider_selection" {
  value = module.evm_cloud.provider_selection
}

output "capability_contract" {
  value = module.evm_cloud.capability_contract
}

output "postgres" {
  value = module.evm_cloud.postgres
}

output "rpc_proxy" {
  value = module.evm_cloud.rpc_proxy
}

output "indexer" {
  value = module.evm_cloud.indexer
}

output "workload_handoff" {
  value = module.evm_cloud.workload_handoff
}
