terraform {
  required_version = ">= 1.14.6"

  required_providers {
    random = {
      source  = "hashicorp/random"
      version = ">= 3.0"
    }
    aws = {
      source  = "hashicorp/aws"
      version = ">= 5.0"
    }
  }
}

provider "aws" {
  region                      = var.aws_region
  skip_credentials_validation = var.aws_skip_credentials_validation
  skip_metadata_api_check     = var.aws_skip_credentials_validation
  skip_requesting_account_id  = var.aws_skip_credentials_validation
}

# Dev: Terraform-managed password with own Secrets Manager secret (recovery_window = 0).
# This avoids the 7-day recovery window of AWS-managed RDS secrets which blocks
# destroy+re-apply cycles. For production, omit postgres_master_password to use
# AWS-managed credentials (automatic rotation, 7-day recovery).
resource "random_password" "rds_master" {
  count   = var.postgres_enabled ? 1 : 0
  length  = 32
  special = false
}

resource "aws_secretsmanager_secret" "rds_master" {
  #checkov:skip=CKV2_AWS_57:Dev example — rotation not required
  count                   = var.postgres_enabled ? 1 : 0
  name                    = "${var.project_name}-rds-master"
  recovery_window_in_days = 0
}

resource "aws_secretsmanager_secret_version" "rds_master" {
  count     = var.postgres_enabled ? 1 : 0
  secret_id = aws_secretsmanager_secret.rds_master[0].id
  secret_string = jsonencode({
    username = var.postgres_db_username
    password = random_password.rds_master[0].result
  })
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
  ssh_private_key_path               = var.ssh_private_key_path
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
  postgres_enabled                     = var.postgres_enabled
  postgres_instance_class              = var.postgres_instance_class
  postgres_engine_version              = var.postgres_engine_version
  postgres_db_name                     = var.postgres_db_name
  postgres_db_username                 = var.postgres_db_username
  postgres_backup_retention            = var.postgres_backup_retention
  postgres_manage_master_user_password = false # Dev: use explicit password for clean destroy
  postgres_master_password             = var.postgres_enabled ? random_password.rds_master[0].result : null

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
  value     = module.evm_cloud.workload_handoff
  sensitive = true
}
