terraform {
  required_version = ">= 1.5.0"
}

module "evm_cloud" {
  source = "../.."

  project_name            = var.project_name
  infrastructure_provider = "aws"
  deployment_target       = "managed"
  runtime_arch            = "multi"
  database_mode           = "self_hosted"
  streaming_mode          = "disabled"

  # Cloudflare ingress — TLS termination via CF origin certificate
  ingress_mode                   = "cloudflare"
  erpc_hostname                  = var.erpc_hostname
  ingress_cloudflare_origin_cert = var.ingress_cloudflare_origin_cert
  ingress_cloudflare_origin_key  = var.ingress_cloudflare_origin_key

  # k3s compute engine — Phase 1 (Terraform) provisions EC2 + installs k3s
  # Phase 2: run deployers/k3s/deploy.sh with the workload_handoff output
  compute_engine = "k3s"
  workload_mode  = "external"

  # SSH keys
  ssh_public_key           = var.ssh_public_key
  k3s_ssh_private_key_path = var.k3s_ssh_private_key_path
  k3s_instance_type        = var.k3s_instance_type
  k3s_version              = var.k3s_version
  k3s_api_allowed_cidrs    = var.k3s_api_allowed_cidrs

  # AWS
  aws_region                      = var.aws_region
  aws_skip_credentials_validation = var.aws_skip_credentials_validation
  networking_enabled              = true
  network_environment             = "dev"
  network_vpc_cidr                = var.network_vpc_cidr
  network_availability_zones      = var.network_availability_zones
  network_enable_nat_gateway      = false
  network_enable_vpc_endpoints    = false

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

output "workload_handoff" {
  value     = module.evm_cloud.workload_handoff
  sensitive = true
}
