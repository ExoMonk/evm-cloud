locals {
  supported_providers = ["aws", "bare_metal"]
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
  count = var.infrastructure_provider == "aws" && var.compute_engine == "eks" ? 1 : 0
  name  = module.provider_aws[0].eks_cluster_name
}

data "aws_eks_cluster_auth" "this" {
  count = var.infrastructure_provider == "aws" && var.compute_engine == "eks" ? 1 : 0
  name  = module.provider_aws[0].eks_cluster_name
}

provider "kubernetes" {
  host                   = try(data.aws_eks_cluster.this[0].endpoint, "")
  cluster_ca_certificate = try(base64decode(data.aws_eks_cluster.this[0].certificate_authority[0].data), "")
  token                  = try(data.aws_eks_cluster_auth.this[0].token, "")
}

# --- Helm provider (EKS) ---
# Mirrors kubernetes provider auth. Inert when compute_engine != "eks".

provider "helm" {
  kubernetes {
    host                   = try(data.aws_eks_cluster.this[0].endpoint, "")
    cluster_ca_certificate = try(base64decode(data.aws_eks_cluster.this[0].certificate_authority[0].data), "")
    token                  = try(data.aws_eks_cluster_auth.this[0].token, "")
  }
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
      error_message = "Unsupported infrastructure_provider. Implemented adapters: aws, bare_metal."
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

    precondition {
      condition     = !(var.infrastructure_provider == "bare_metal" && !contains(["docker_compose", "k3s"], var.compute_engine))
      error_message = "bare_metal requires compute_engine=docker_compose or compute_engine=k3s."
    }

    precondition {
      condition     = !(var.infrastructure_provider == "aws" && !contains(["ec2", "eks", "k3s"], var.compute_engine))
      error_message = "aws requires compute_engine=ec2, compute_engine=eks, or compute_engine=k3s."
    }

    precondition {
      condition     = !(var.compute_engine == "k3s" && var.workload_mode != "external")
      error_message = "k3s requires workload_mode=external. Use deployers/k3s/ for workload deployment."
    }

    precondition {
      condition     = !(var.compute_engine == "k3s" && var.infrastructure_provider == "aws" && var.ssh_public_key == "")
      error_message = "ssh_public_key is required when compute_engine=k3s on AWS (needed for EC2 k3s host)."
    }

    precondition {
      condition     = !(var.compute_engine == "k3s" && var.infrastructure_provider == "aws" && var.k3s_ssh_private_key_path == "")
      error_message = "k3s_ssh_private_key_path is required when compute_engine=k3s on AWS (needed for SSH provisioning)."
    }

    precondition {
      condition     = !(var.infrastructure_provider == "bare_metal" && var.bare_metal_host == "")
      error_message = "bare_metal_host is required when infrastructure_provider=bare_metal."
    }

    precondition {
      condition     = !(var.infrastructure_provider == "bare_metal" && var.bare_metal_ssh_private_key_path == "")
      error_message = "bare_metal_ssh_private_key_path is required when infrastructure_provider=bare_metal."
    }

  }
}

module "provider_aws" {
  source = "./modules/providers/aws"
  count  = var.infrastructure_provider == "aws" ? 1 : 0

  providers = {
    aws        = aws
    kubernetes = kubernetes
    helm       = helm
  }

  project_name                       = var.project_name
  deployment_target                  = var.deployment_target
  runtime_arch                       = var.runtime_arch
  database_mode                      = var.database_mode
  streaming_mode                     = var.streaming_mode
  ingress_mode                       = var.ingress_mode
  compute_engine                     = var.compute_engine
  workload_mode                      = var.workload_mode
  ssh_public_key                     = var.ssh_public_key
  ec2_instance_type                  = var.ec2_instance_type
  ec2_rpc_proxy_mem_limit            = var.ec2_rpc_proxy_mem_limit
  ec2_indexer_mem_limit              = var.ec2_indexer_mem_limit
  ec2_secret_recovery_window_in_days = var.ec2_secret_recovery_window_in_days

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
  indexer_instances       = var.indexer_instances

  # ClickHouse BYODB
  indexer_clickhouse_url      = var.indexer_clickhouse_url
  indexer_clickhouse_user     = var.indexer_clickhouse_user
  indexer_clickhouse_password = var.indexer_clickhouse_password
  indexer_clickhouse_db       = var.indexer_clickhouse_db

  # Config injection
  erpc_config_yaml     = var.erpc_config_yaml
  rindexer_config_yaml = var.rindexer_config_yaml
  rindexer_abis        = var.rindexer_abis

  # k3s
  k3s_version              = var.k3s_version
  k3s_instance_type        = var.k3s_instance_type
  k3s_api_allowed_cidrs    = var.k3s_api_allowed_cidrs
  k3s_ssh_private_key_path = var.k3s_ssh_private_key_path
  k3s_worker_nodes         = var.k3s_worker_nodes
}

module "provider_bare_metal" {
  source = "./modules/providers/bare_metal"
  count  = var.infrastructure_provider == "bare_metal" ? 1 : 0

  project_name   = var.project_name
  compute_engine = var.compute_engine
  workload_mode  = var.workload_mode

  # SSH connection
  host_address         = var.bare_metal_host
  ssh_user             = var.bare_metal_ssh_user
  ssh_private_key_path = var.bare_metal_ssh_private_key_path
  ssh_port             = var.bare_metal_ssh_port

  # RPC Proxy
  rpc_proxy_enabled   = var.rpc_proxy_enabled
  rpc_proxy_image     = var.rpc_proxy_image
  rpc_proxy_mem_limit = var.bare_metal_rpc_proxy_mem_limit
  erpc_config_yaml    = var.erpc_config_yaml

  # Indexer
  indexer_enabled         = var.indexer_enabled
  indexer_image           = var.indexer_image
  indexer_rpc_url         = var.indexer_rpc_url
  indexer_storage_backend = var.indexer_storage_backend
  indexer_instances       = var.indexer_instances
  indexer_mem_limit       = var.bare_metal_indexer_mem_limit
  rindexer_config_yaml    = var.rindexer_config_yaml
  rindexer_abis           = var.rindexer_abis

  # ClickHouse BYODB
  indexer_clickhouse_url      = var.indexer_clickhouse_url
  indexer_clickhouse_user     = var.indexer_clickhouse_user
  indexer_clickhouse_password = var.indexer_clickhouse_password
  indexer_clickhouse_db       = var.indexer_clickhouse_db

  # k3s
  k3s_version      = var.k3s_version
  k3s_worker_nodes = var.k3s_worker_nodes
}
