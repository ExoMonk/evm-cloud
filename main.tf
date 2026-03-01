locals {
  supported_providers = ["aws"]
}

provider "aws" {
  region                      = var.aws_region
  skip_credentials_validation = var.aws_skip_credentials_validation
  skip_metadata_api_check     = var.aws_skip_credentials_validation
  skip_requesting_account_id  = var.aws_skip_credentials_validation
}

# --- Kubernetes provider (EKS) ---
# Provider config must live at root — Terraform limitation.

data "aws_eks_cluster" "this" {
  count = var.compute_engine == "eks" ? 1 : 0
  name  = module.provider_aws[0].eks_cluster_name
}

data "aws_eks_cluster_auth" "this" {
  count = var.compute_engine == "eks" ? 1 : 0
  name  = module.provider_aws[0].eks_cluster_name
}

provider "kubernetes" {
  host                   = try(data.aws_eks_cluster.this[0].endpoint, "")
  cluster_ca_certificate = try(base64decode(data.aws_eks_cluster.this[0].certificate_authority[0].data), "")
  token                  = try(data.aws_eks_cluster_auth.this[0].token, "")
}

module "capabilities" {
  source = "./modules/core/capabilities"

  infrastructure_provider = var.infrastructure_provider
  deployment_target       = var.deployment_target
  runtime_arch            = var.runtime_arch
  database_mode           = var.database_mode
  streaming_mode          = var.streaming_mode
  ingress_mode            = var.ingress_mode
  compute_engine          = var.compute_engine
  workload_mode           = var.workload_mode
}

resource "terraform_data" "provider_guardrails" {
  input = {
    provider = var.infrastructure_provider
  }

  lifecycle {
    precondition {
      condition     = contains(local.supported_providers, var.infrastructure_provider)
      error_message = "Unsupported infrastructure_provider. Implemented adapters: aws. Add modules/providers/<provider> before using a different value."
    }

    precondition {
      condition     = !(var.infrastructure_provider != "aws" && var.ingress_mode == "managed_lb")
      error_message = "ingress_mode=managed_lb currently requires infrastructure_provider=aws."
    }

    precondition {
      condition     = !(var.infrastructure_provider != "aws" && var.database_mode == "managed")
      error_message = "database_mode=managed currently requires infrastructure_provider=aws."
    }

    precondition {
      condition     = !(var.infrastructure_provider != "aws" && var.streaming_mode == "managed")
      error_message = "streaming_mode=managed currently requires infrastructure_provider=aws."
    }
  }
}

module "provider_aws" {
  source = "./modules/providers/aws"
  count  = var.infrastructure_provider == "aws" ? 1 : 0

  project_name      = var.project_name
  deployment_target = var.deployment_target
  runtime_arch      = var.runtime_arch
  database_mode     = var.database_mode
  streaming_mode    = var.streaming_mode
  ingress_mode      = var.ingress_mode
  compute_engine    = var.compute_engine
  workload_mode     = var.workload_mode

  aws_region                   = var.aws_region
  networking_enabled           = var.networking_enabled
  network_environment          = var.network_environment
  network_vpc_cidr             = var.network_vpc_cidr
  network_availability_zones   = var.network_availability_zones
  network_enable_nat_gateway   = var.network_enable_nat_gateway
  network_enable_vpc_endpoints = var.network_enable_vpc_endpoints

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

  # ClickHouse BYODB
  indexer_clickhouse_url      = var.indexer_clickhouse_url
  indexer_clickhouse_user     = var.indexer_clickhouse_user
  indexer_clickhouse_password = var.indexer_clickhouse_password
  indexer_clickhouse_db       = var.indexer_clickhouse_db

  # Config injection
  erpc_config_yaml     = var.erpc_config_yaml
  rindexer_config_yaml = var.rindexer_config_yaml
  rindexer_abis        = var.rindexer_abis
}
