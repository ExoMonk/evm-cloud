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
      condition     = !(var.ingress_mode == "cloudflare" && var.ingress_cloudflare_origin_cert == "")
      error_message = "ingress_cloudflare_origin_cert is required when ingress_mode = cloudflare. Generate at Cloudflare dashboard > SSL/TLS > Origin Server."
    }

    precondition {
      condition     = !(var.ingress_mode == "cloudflare" && var.ingress_cloudflare_origin_key == "")
      error_message = "ingress_cloudflare_origin_key is required when ingress_mode = cloudflare."
    }

    precondition {
      condition     = !(var.ingress_mode == "caddy" && var.ingress_tls_email == "")
      error_message = "ingress_tls_email is required when ingress_mode = caddy (needed for Let's Encrypt)."
    }

    precondition {
      condition     = !(var.ingress_mode == "ingress_nginx" && var.ingress_tls_email == "")
      error_message = "ingress_tls_email is required when ingress_mode = ingress_nginx (needed for cert-manager)."
    }

    precondition {
      condition     = !(var.ingress_mode == "caddy" && !contains(["ec2", "docker_compose"], var.compute_engine))
      error_message = "ingress_mode = caddy requires compute_engine = ec2 or docker_compose."
    }

    precondition {
      condition     = !(var.ingress_mode == "ingress_nginx" && !contains(["k3s", "eks"], var.compute_engine))
      error_message = "ingress_mode = ingress_nginx requires compute_engine = k3s or eks."
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
      condition     = !(contains(["ec2", "k3s"], var.compute_engine) && var.ssh_private_key_path == "") && !(var.infrastructure_provider == "bare_metal" && var.ssh_private_key_path == "")
      error_message = "ssh_private_key_path is required for EC2, K3s, and bare metal deployments."
    }

    precondition {
      condition     = !(var.infrastructure_provider == "bare_metal" && var.bare_metal_host == "")
      error_message = "bare_metal_host is required when infrastructure_provider=bare_metal."
    }

    precondition {
      condition     = !(var.secrets_mode == "provider" && var.infrastructure_provider != "aws")
      error_message = "secrets_mode=provider requires infrastructure_provider=aws (uses AWS Secrets Manager)."
    }

    precondition {
      condition     = !(var.secrets_mode == "external" && var.external_secret_store_name == "")
      error_message = "external_secret_store_name is required when secrets_mode=external."
    }

    precondition {
      condition     = !(var.secrets_mode == "external" && var.external_secret_key == "")
      error_message = "external_secret_key is required when secrets_mode=external."
    }

    precondition {
      condition     = !(var.bare_metal_secrets_encryption == "sops_age")
      error_message = "bare_metal_secrets_encryption=sops_age is not yet implemented. Use secrets_mode=external with ESO for bare_metal k3s secret management."
    }

    precondition {
      condition     = !(var.monitoring_enabled && !contains(["eks", "k3s"], var.compute_engine))
      error_message = "monitoring_enabled requires compute_engine = eks or k3s."
    }

    precondition {
      condition     = !(var.monitoring_enabled && var.grafana_ingress_enabled && var.grafana_hostname == "")
      error_message = "grafana_hostname is required when monitoring_enabled and grafana_ingress_enabled are both true."
    }

    precondition {
      condition     = !(var.loki_enabled && !var.monitoring_enabled)
      error_message = "loki_enabled requires monitoring_enabled = true."
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
  ssh_private_key_path               = var.ssh_private_key_path
  ec2_secret_recovery_window_in_days = var.ec2_secret_recovery_window_in_days

  aws_region                   = var.aws_region
  networking_enabled           = var.networking_enabled
  network_environment          = var.network_environment
  network_vpc_cidr             = var.network_vpc_cidr
  network_availability_zones   = var.network_availability_zones
  network_enable_nat_gateway   = var.network_enable_nat_gateway
  network_enable_vpc_endpoints = var.network_enable_vpc_endpoints

  # Postgres
  postgres_enabled                     = var.postgres_enabled
  postgres_instance_class              = var.postgres_instance_class
  postgres_engine_version              = var.postgres_engine_version
  postgres_db_name                     = var.postgres_db_name
  postgres_db_username                 = var.postgres_db_username
  postgres_backup_retention            = var.postgres_backup_retention
  postgres_manage_master_user_password = var.postgres_manage_master_user_password
  postgres_master_password             = var.postgres_master_password
  postgres_force_ssl                   = var.postgres_force_ssl

  # RPC Proxy
  rpc_proxy_enabled = var.rpc_proxy_enabled
  rpc_proxy_image   = var.rpc_proxy_image

  # Indexer
  indexer_enabled          = var.indexer_enabled
  indexer_image            = var.indexer_image
  indexer_rpc_url          = var.indexer_rpc_url
  indexer_storage_backend  = var.indexer_storage_backend
  indexer_instances        = var.indexer_instances
  indexer_extra_env        = var.indexer_extra_env
  indexer_extra_secret_env = var.indexer_extra_secret_env

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
  k3s_version           = var.k3s_version
  k3s_instance_type     = var.k3s_instance_type
  k3s_api_allowed_cidrs = var.k3s_api_allowed_cidrs
  k3s_worker_nodes      = var.k3s_worker_nodes

  # Ingress / TLS
  erpc_hostname                      = var.erpc_hostname
  ingress_tls_email                  = var.ingress_tls_email
  ingress_cloudflare_origin_cert     = var.ingress_cloudflare_origin_cert
  ingress_cloudflare_origin_key      = var.ingress_cloudflare_origin_key
  ingress_cloudflare_ssl_mode        = var.ingress_cloudflare_ssl_mode
  ingress_caddy_image                = var.ingress_caddy_image
  ingress_caddy_mem_limit            = var.ingress_caddy_mem_limit
  ingress_nginx_chart_version        = var.ingress_nginx_chart_version
  ingress_cert_manager_chart_version = var.ingress_cert_manager_chart_version
  ingress_request_body_max_size      = var.ingress_request_body_max_size
  ingress_tls_staging                = var.ingress_tls_staging
  ingress_hsts_preload               = var.ingress_hsts_preload

  # Secrets
  secrets_mode               = var.secrets_mode
  secrets_manager_secret_arn = var.secrets_manager_secret_arn
  secrets_manager_kms_key_id = var.secrets_manager_kms_key_id
  external_secret_store_name = var.external_secret_store_name
  external_secret_key        = var.external_secret_key
  eso_chart_version          = var.eso_chart_version

  # Monitoring
  monitoring_enabled                             = var.monitoring_enabled
  kube_prometheus_stack_version                  = var.kube_prometheus_stack_version
  grafana_admin_password_secret_name             = var.grafana_admin_password_secret_name
  alertmanager_slack_webhook_secret_name         = var.alertmanager_slack_webhook_secret_name
  alertmanager_sns_topic_arn                     = var.alertmanager_sns_topic_arn
  alertmanager_pagerduty_routing_key_secret_name = var.alertmanager_pagerduty_routing_key_secret_name
  alertmanager_route_target                      = var.alertmanager_route_target
  alertmanager_slack_channel                     = var.alertmanager_slack_channel
  loki_enabled                                   = var.loki_enabled
  loki_chart_version                             = var.loki_chart_version
  promtail_chart_version                         = var.promtail_chart_version
  loki_persistence_enabled                       = var.loki_persistence_enabled
  clickhouse_metrics_url                         = var.clickhouse_metrics_url
  grafana_ingress_enabled                        = var.grafana_ingress_enabled
  grafana_hostname                               = var.grafana_hostname
  ingress_class_name                             = var.ingress_class_name
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
  ssh_private_key_path = var.ssh_private_key_path
  ssh_port             = var.bare_metal_ssh_port

  # RPC Proxy
  rpc_proxy_enabled   = var.rpc_proxy_enabled
  rpc_proxy_image     = var.rpc_proxy_image
  rpc_proxy_mem_limit = var.bare_metal_rpc_proxy_mem_limit
  erpc_config_yaml    = var.erpc_config_yaml

  # Indexer
  indexer_enabled          = var.indexer_enabled
  indexer_image            = var.indexer_image
  indexer_rpc_url          = var.indexer_rpc_url
  indexer_storage_backend  = var.indexer_storage_backend
  indexer_instances        = var.indexer_instances
  indexer_extra_env        = var.indexer_extra_env
  indexer_extra_secret_env = var.indexer_extra_secret_env
  indexer_mem_limit        = var.bare_metal_indexer_mem_limit
  rindexer_config_yaml     = var.rindexer_config_yaml
  rindexer_abis            = var.rindexer_abis

  # ClickHouse BYODB
  indexer_clickhouse_url      = var.indexer_clickhouse_url
  indexer_clickhouse_user     = var.indexer_clickhouse_user
  indexer_clickhouse_password = var.indexer_clickhouse_password
  indexer_clickhouse_db       = var.indexer_clickhouse_db

  # PostgreSQL BYODB
  indexer_postgres_url = var.indexer_postgres_url

  # k3s
  k3s_version      = var.k3s_version
  k3s_worker_nodes = var.k3s_worker_nodes

  # Ingress / TLS
  ingress_mode                       = var.ingress_mode
  erpc_hostname                      = var.erpc_hostname
  ingress_tls_email                  = var.ingress_tls_email
  ingress_cloudflare_origin_cert     = var.ingress_cloudflare_origin_cert
  ingress_cloudflare_origin_key      = var.ingress_cloudflare_origin_key
  ingress_cloudflare_ssl_mode        = var.ingress_cloudflare_ssl_mode
  ingress_caddy_image                = var.ingress_caddy_image
  ingress_caddy_mem_limit            = var.ingress_caddy_mem_limit
  ingress_nginx_chart_version        = var.ingress_nginx_chart_version
  ingress_cert_manager_chart_version = var.ingress_cert_manager_chart_version
  ingress_request_body_max_size      = var.ingress_request_body_max_size
  ingress_tls_staging                = var.ingress_tls_staging
  ingress_hsts_preload               = var.ingress_hsts_preload

  # Secrets
  secrets_mode               = var.secrets_mode
  external_secret_store_name = var.external_secret_store_name
  external_secret_key        = var.external_secret_key
  eso_chart_version          = var.eso_chart_version

  # Monitoring
  monitoring_enabled                             = var.monitoring_enabled
  kube_prometheus_stack_version                  = var.kube_prometheus_stack_version
  grafana_admin_password_secret_name             = var.grafana_admin_password_secret_name
  alertmanager_slack_webhook_secret_name         = var.alertmanager_slack_webhook_secret_name
  alertmanager_sns_topic_arn                     = var.alertmanager_sns_topic_arn
  alertmanager_pagerduty_routing_key_secret_name = var.alertmanager_pagerduty_routing_key_secret_name
  alertmanager_route_target                      = var.alertmanager_route_target
  alertmanager_slack_channel                     = var.alertmanager_slack_channel
  loki_enabled                                   = var.loki_enabled
  loki_chart_version                             = var.loki_chart_version
  promtail_chart_version                         = var.promtail_chart_version
  loki_persistence_enabled                       = var.loki_persistence_enabled
  clickhouse_metrics_url                         = var.clickhouse_metrics_url
  grafana_ingress_enabled                        = var.grafana_ingress_enabled
  grafana_hostname                               = var.grafana_hostname
  ingress_class_name                             = var.ingress_class_name
}
