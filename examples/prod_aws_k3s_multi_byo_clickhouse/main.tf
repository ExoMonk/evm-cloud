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
  ingress_mode            = "self_hosted"

  # k3s compute engine — Phase 1 (Terraform) provisions EC2 + installs k3s
  # Phase 2: run deployers/k3s/deploy.sh with the workload_handoff output
  compute_engine = "k3s"
  workload_mode  = "external"

  # SSH keys — public key for EC2, private key path for k3s provisioner
  ssh_public_key           = var.ssh_public_key
  k3s_ssh_private_key_path = var.k3s_ssh_private_key_path
  k3s_instance_type        = var.k3s_instance_type
  k3s_version              = var.k3s_version
  k3s_api_allowed_cidrs    = var.k3s_api_allowed_cidrs # Must include your IP for SSH + k3s provisioning
  k3s_worker_nodes         = var.k3s_worker_nodes

  # AWS
  aws_region                      = var.aws_region
  aws_skip_credentials_validation = var.aws_skip_credentials_validation
  networking_enabled              = true
  network_environment             = "dev"
  network_vpc_cidr                = var.network_vpc_cidr
  network_availability_zones      = var.network_availability_zones
  network_enable_nat_gateway      = false # k3s host is on public subnet with public IP — no NAT needed
  network_enable_vpc_endpoints    = false

  # RPC Proxy — config will be deployed via Helm in Phase 2
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

  # Multi-instance indexer — live on server, backfill on worker node
  indexer_instances = var.indexer_instances

  # Secrets — provider mode: AWS Secrets Manager + ESO (no passwords in handoff)
  secrets_mode                       = var.secrets_mode
  secrets_manager_secret_arn         = var.secrets_manager_secret_arn
  secrets_manager_kms_key_id         = var.secrets_manager_kms_key_id
  ec2_secret_recovery_window_in_days = var.ec2_secret_recovery_window_in_days

  # Config injection — used by workload_handoff for the deployer
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
