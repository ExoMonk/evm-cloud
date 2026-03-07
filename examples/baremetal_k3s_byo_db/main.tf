terraform {
  required_version = ">= 1.5.0"
}

module "evm_cloud" {
  source = "../.."

  project_name            = var.project_name
  infrastructure_provider = "bare_metal"
  deployment_target       = "self_hosted"
  runtime_arch            = "multi"
  database_mode           = "self_hosted"
  streaming_mode          = "disabled"
  ingress_mode            = "none"

  # k3s compute engine — Phase 1 (Terraform) provisions k3s on existing VPS
  # Phase 2: run deployers/k3s/deploy.sh with the workload_handoff output
  compute_engine = "k3s"
  workload_mode  = "external"

  # Bare metal SSH connection
  bare_metal_host      = var.bare_metal_host
  bare_metal_ssh_user  = var.bare_metal_ssh_user
  ssh_private_key_path = var.ssh_private_key_path
  bare_metal_ssh_port  = var.bare_metal_ssh_port

  # k3s
  k3s_version = var.k3s_version

  # RPC Proxy — config deployed via Helm in Phase 2
  rpc_proxy_enabled = var.rpc_proxy_enabled
  rpc_proxy_image   = var.rpc_proxy_image

  # Indexer — PostgreSQL BYODB
  indexer_enabled         = var.indexer_enabled
  indexer_image           = var.indexer_image
  indexer_rpc_url         = var.indexer_rpc_url
  indexer_storage_backend = "postgres"
  indexer_postgres_url    = var.indexer_postgres_url

  # Config injection — used by workload_handoff for the deployer
  erpc_config_yaml     = file("${path.module}/config/erpc.yaml")
  rindexer_config_yaml = file("${path.module}/config/rindexer.yaml")
  rindexer_abis = {
    "ERC20.json" = file("${path.module}/config/abis/ERC20.json")
  }

  # Secrets — inline mode (simplest, DATABASE_URL flows through handoff)
  # For production with a secret backend, see prod_aws_k3s_multi_byo_clickhouse example
  secrets_mode = "inline"
}

output "provider_selection" {
  value = module.evm_cloud.provider_selection
}

output "capability_contract" {
  value = module.evm_cloud.capability_contract
}

output "workload_handoff" {
  value     = module.evm_cloud.workload_handoff
  sensitive = true
}
