terraform {
  required_version = ">= 1.14.6"
}

module "evm_cloud" {
  source = "../.."

  project_name            = var.project_name
  infrastructure_provider = "bare_metal"
  deployment_target       = "self_hosted"
  database_mode           = "self_hosted"
  streaming_mode          = "disabled"
  ingress_mode            = "self_hosted"
  compute_engine          = "docker_compose"
  workload_mode           = var.workload_mode

  # Bare metal SSH connection
  bare_metal_host                 = var.bare_metal_host
  bare_metal_ssh_user             = var.bare_metal_ssh_user
  bare_metal_ssh_private_key_path = var.bare_metal_ssh_private_key_path
  bare_metal_ssh_port             = var.bare_metal_ssh_port
  bare_metal_rpc_proxy_mem_limit  = var.bare_metal_rpc_proxy_mem_limit
  bare_metal_indexer_mem_limit    = var.bare_metal_indexer_mem_limit

  # AWS provider (required by Terraform even when unused — credentials skipped)
  aws_skip_credentials_validation = true

  # RPC Proxy
  rpc_proxy_enabled = var.rpc_proxy_enabled
  rpc_proxy_image   = var.rpc_proxy_image

  # Indexer — ClickHouse BYODB
  indexer_enabled         = var.indexer_enabled
  indexer_image           = var.indexer_image
  indexer_rpc_url         = var.indexer_rpc_url
  indexer_storage_backend = "clickhouse"

  indexer_clickhouse_url      = var.indexer_clickhouse_url
  indexer_clickhouse_user     = var.indexer_clickhouse_user
  indexer_clickhouse_password = var.indexer_clickhouse_password
  indexer_clickhouse_db       = var.indexer_clickhouse_db

  # Config injection
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

output "rpc_proxy" {
  value = module.evm_cloud.rpc_proxy
}

output "indexer" {
  value = module.evm_cloud.indexer
}

output "workload_handoff" {
  value     = module.evm_cloud.workload_handoff
  sensitive = true
}
