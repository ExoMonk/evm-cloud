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
  compute_engine          = var.compute_engine
  workload_mode           = var.workload_mode
  ssh_public_key          = var.ssh_public_key
  ec2_instance_type       = var.ec2_instance_type

  aws_region                      = var.aws_region
  aws_skip_credentials_validation = var.aws_skip_credentials_validation
  networking_enabled              = var.networking_enabled
  network_environment             = var.network_environment
  network_vpc_cidr                = var.network_vpc_cidr
  network_availability_zones      = var.network_availability_zones
  network_enable_nat_gateway      = var.network_enable_nat_gateway
  network_enable_vpc_endpoints    = var.network_enable_vpc_endpoints

  rpc_proxy_enabled = var.rpc_proxy_enabled
  rpc_proxy_image   = var.rpc_proxy_image

  indexer_enabled         = var.indexer_enabled
  indexer_image           = var.indexer_image
  indexer_rpc_url         = var.indexer_rpc_url
  indexer_storage_backend = "clickhouse"

  indexer_clickhouse_url      = var.indexer_clickhouse_url
  indexer_clickhouse_user     = var.indexer_clickhouse_user
  indexer_clickhouse_password = var.indexer_clickhouse_password
  indexer_clickhouse_db       = var.indexer_clickhouse_db

  # External mode does not require config payloads at Terraform apply time.
  erpc_config_yaml     = var.erpc_config_yaml
  rindexer_config_yaml = var.rindexer_config_yaml
  rindexer_abis        = var.rindexer_abis
}

output "provider_selection" {
  value = module.evm_cloud.provider_selection
}

output "capability_contract" {
  value = module.evm_cloud.capability_contract
}

output "workload_handoff" {
  value = module.evm_cloud.workload_handoff
}
