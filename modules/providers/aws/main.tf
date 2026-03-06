module "networking" {
  source = "./networking"
  count  = var.networking_enabled ? 1 : 0

  project_name         = var.project_name
  environment          = var.network_environment
  vpc_cidr             = var.network_vpc_cidr
  availability_zones   = var.network_availability_zones
  enable_nat_gateway   = var.network_enable_nat_gateway
  enable_vpc_endpoints = var.network_enable_vpc_endpoints
  ingress_mode         = var.ingress_mode
}

locals {
  networking = var.networking_enabled ? {
    vpc_id             = module.networking[0].vpc_id
    public_subnet_ids  = module.networking[0].public_subnet_ids
    private_subnet_ids = module.networking[0].private_subnet_ids
    security_group_ids = module.networking[0].security_group_ids
  } : null

  any_compute_enabled         = var.rpc_proxy_enabled || var.indexer_enabled
  any_ec2_compute             = local.any_compute_enabled && var.compute_engine == "ec2"
  any_eks_compute             = local.any_compute_enabled && var.compute_engine == "eks"
  any_k3s_compute             = var.compute_engine == "k3s"
  terraform_manages_workloads = var.workload_mode == "terraform"

  # Auto-wire indexer → eRPC when both are enabled and user didn't provide an explicit URL.
  # EC2: Docker Compose service name. EKS: Kubernetes service DNS.
  erpc_internal_url = (
    var.rpc_proxy_enabled && var.compute_engine == "ec2"
    ? "http://erpc:4000"
    : var.rpc_proxy_enabled && var.compute_engine == "eks"
    ? "http://${var.project_name}-erpc:4000"
    : ""
  )
  resolved_indexer_rpc_url = var.indexer_rpc_url != "" ? var.indexer_rpc_url : local.erpc_internal_url

  common_tags = {
    Project     = var.project_name
    Environment = var.network_environment
    ManagedBy   = "terraform"
    Module      = "providers/aws"
  }

  adapter_context = {
    provider          = "aws"
    project_name      = var.project_name
    deployment_target = var.deployment_target
    runtime_arch      = var.runtime_arch
    database_mode     = var.database_mode
    streaming_mode    = var.streaming_mode
    ingress_mode      = var.ingress_mode
    aws_region        = var.aws_region
    networking        = local.networking
  }
}

# --- Guardrails ---

resource "terraform_data" "compute_requires_networking" {
  count = local.any_compute_enabled ? 1 : 0

  lifecycle {
    precondition {
      condition     = var.networking_enabled
      error_message = "rpc_proxy_enabled and indexer_enabled require networking_enabled=true."
    }
  }
}

resource "terraform_data" "k3s_requires_networking" {
  count = local.any_k3s_compute ? 1 : 0

  lifecycle {
    precondition {
      condition     = var.networking_enabled
      error_message = "compute_engine=k3s requires networking_enabled=true (VPC for EC2 host)."
    }
  }
}

resource "terraform_data" "ec2_requires_ssh_key" {
  count = local.any_ec2_compute ? 1 : 0

  lifecycle {
    precondition {
      condition     = var.ssh_public_key != ""
      error_message = "ssh_public_key is required when compute_engine=ec2."
    }
  }
}

resource "terraform_data" "postgres_requires_networking" {
  count = var.postgres_enabled ? 1 : 0

  lifecycle {
    precondition {
      condition     = var.networking_enabled
      error_message = "postgres_enabled requires networking_enabled=true."
    }
  }
}

resource "terraform_data" "indexer_requires_postgres" {
  count = (var.indexer_enabled && var.indexer_storage_backend == "postgres") ? 1 : 0

  lifecycle {
    precondition {
      condition     = var.postgres_enabled
      error_message = "indexer with storage_backend=postgres requires postgres_enabled=true."
    }
  }
}

resource "terraform_data" "indexer_clickhouse_requires_url" {
  count = (var.indexer_enabled && var.indexer_storage_backend == "clickhouse") ? 1 : 0

  lifecycle {
    precondition {
      condition     = var.indexer_clickhouse_url != ""
      error_message = "indexer_clickhouse_url is required when indexer_storage_backend=clickhouse."
    }
  }
}

resource "terraform_data" "rpc_proxy_requires_config" {
  count = (var.rpc_proxy_enabled && local.terraform_manages_workloads) ? 1 : 0

  lifecycle {
    precondition {
      condition     = var.erpc_config_yaml != ""
      error_message = "erpc_config_yaml is required when rpc_proxy_enabled=true. eRPC reads erpc.yaml, not env vars."
    }
  }
}

resource "terraform_data" "indexer_requires_config" {
  count = (var.indexer_enabled && local.terraform_manages_workloads) ? 1 : 0

  lifecycle {
    precondition {
      condition     = var.rindexer_config_yaml != ""
      error_message = "rindexer_config_yaml is required when indexer_enabled=true. rindexer needs rindexer.yaml to boot."
    }
  }
}

# --- Database: PostgreSQL ---

module "postgres" {
  source = "./database/postgres"
  count  = var.postgres_enabled ? 1 : 0

  project_name      = var.project_name
  environment       = var.network_environment
  subnet_ids        = local.networking.private_subnet_ids
  security_group_id = local.networking.security_group_ids["database"]

  engine_version              = var.postgres_engine_version
  instance_class              = var.postgres_instance_class
  db_name                     = var.postgres_db_name
  db_username                 = var.postgres_db_username
  backup_retention_period     = var.postgres_backup_retention
  manage_master_user_password = var.postgres_manage_master_user_password
  master_password             = var.postgres_master_password
  force_ssl                   = var.postgres_force_ssl
}

# --- EC2+Docker Compose ---

# Resolve Postgres credentials for EC2 secret payload (only when AWS manages the secret)
data "aws_secretsmanager_secret_version" "postgres_master_ec2" {
  count     = (var.indexer_enabled && var.compute_engine == "ec2" && var.indexer_storage_backend == "postgres" && var.postgres_enabled && var.postgres_manage_master_user_password) ? 1 : 0
  secret_id = module.postgres[0].master_secret_arn
}

module "ec2" {
  source = "./ec2"
  count  = local.any_ec2_compute ? 1 : 0

  project_name                  = var.project_name
  workload_mode                 = var.workload_mode
  environment                   = var.network_environment
  subnet_id                     = local.networking.public_subnet_ids[0]
  security_group_id             = local.networking.security_group_ids["ec2"]
  additional_security_group_ids = [local.networking.security_group_ids["indexer"]]
  instance_profile_name         = aws_iam_instance_profile.ec2[0].name
  ssh_public_key                = var.ssh_public_key
  ssh_private_key_path          = var.ssh_private_key_path
  instance_type                 = var.ec2_instance_type
  aws_region                    = var.aws_region
  tags                          = local.common_tags

  # Ingress / TLS
  ingress_mode                   = var.ingress_mode
  erpc_hostname                  = var.erpc_hostname
  ingress_tls_email              = var.ingress_tls_email
  ingress_cloudflare_origin_cert = var.ingress_cloudflare_origin_cert
  ingress_cloudflare_origin_key  = var.ingress_cloudflare_origin_key
  ingress_caddy_image            = var.ingress_caddy_image
  ingress_caddy_mem_limit        = var.ingress_caddy_mem_limit
  ingress_request_body_max_size  = var.ingress_request_body_max_size
  ingress_tls_staging            = var.ingress_tls_staging
  ingress_hsts_preload           = var.ingress_hsts_preload

  enable_rpc_proxy               = var.rpc_proxy_enabled
  enable_indexer                 = var.indexer_enabled
  rpc_proxy_image                = var.rpc_proxy_image
  indexer_image                  = var.indexer_image
  rpc_url                        = local.resolved_indexer_rpc_url
  rpc_proxy_mem_limit            = var.ec2_rpc_proxy_mem_limit
  indexer_mem_limit              = var.ec2_indexer_mem_limit
  secret_recovery_window_in_days = var.ec2_secret_recovery_window_in_days

  erpc_yaml_content     = var.erpc_config_yaml
  rindexer_yaml_content = var.rindexer_config_yaml
  abi_files             = var.rindexer_abis

  storage_backend = var.indexer_storage_backend

  # Postgres: compose DATABASE_URL from RDS secret
  db_host = var.indexer_storage_backend == "postgres" && var.postgres_enabled ? module.postgres[0].endpoint : ""
  db_port = var.indexer_storage_backend == "postgres" && var.postgres_enabled ? module.postgres[0].port : 5432
  db_name = var.indexer_storage_backend == "postgres" && var.postgres_enabled ? module.postgres[0].db_name : ""
  db_username = var.indexer_storage_backend == "postgres" && var.postgres_enabled ? (
    !var.postgres_manage_master_user_password ? module.postgres[0].master_username : try(jsondecode(data.aws_secretsmanager_secret_version.postgres_master_ec2[0].secret_string)["username"], "")
  ) : ""
  db_password = var.indexer_storage_backend == "postgres" && var.postgres_enabled ? (
    !var.postgres_manage_master_user_password ? var.postgres_master_password : try(jsondecode(data.aws_secretsmanager_secret_version.postgres_master_ec2[0].secret_string)["password"], "")
  ) : ""

  # ClickHouse BYODB
  clickhouse_url      = var.indexer_clickhouse_url
  clickhouse_user     = var.indexer_clickhouse_user
  clickhouse_password = var.indexer_clickhouse_password
  clickhouse_db       = var.indexer_clickhouse_db
}

# --- EKS Cluster ---

module "eks_cluster" {
  source = "./eks/cluster"
  count  = local.any_eks_compute ? 1 : 0

  project_name = var.project_name
  environment  = var.network_environment
  vpc_id       = local.networking.vpc_id
  subnet_ids   = local.networking.private_subnet_ids
  common_tags  = local.common_tags
}

# --- k3s Host + Bootstrap ---

module "k3s_host" {
  source = "./k3s-host"
  count  = local.any_k3s_compute ? 1 : 0

  project_name                  = var.project_name
  environment                   = var.network_environment
  instance_type                 = var.k3s_instance_type
  subnet_id                     = local.networking.public_subnet_ids[0]
  vpc_id                        = local.networking.vpc_id
  vpc_cidr                      = var.network_vpc_cidr
  ssh_public_key                = var.ssh_public_key
  k3s_api_allowed_cidrs         = var.k3s_api_allowed_cidrs
  additional_security_group_ids = [local.networking.security_group_ids["indexer"]]
  tags                          = local.common_tags
  ingress_mode                  = var.ingress_mode
  secrets_mode                  = var.secrets_mode
  secrets_manager_prefix        = "evm-cloud/${var.project_name}"
  secrets_manager_secret_arn    = local.workload_secret_arn
}

module "k3s_bootstrap" {
  source     = "../../core/k8s/k3s-bootstrap"
  count      = local.any_k3s_compute ? 1 : 0
  depends_on = [module.k3s_host]

  host_address         = module.k3s_host[0].host_ip
  ssh_user             = module.k3s_host[0].ssh_user
  ssh_private_key_path = var.ssh_private_key_path
  project_name         = var.project_name
  k3s_version          = var.k3s_version
  tls_san_entries      = [module.k3s_host[0].host_ip]
}

# --- k3s Worker Nodes ---

module "k3s_worker_host" {
  source   = "./k3s-host"
  for_each = { for node in var.k3s_worker_nodes : node.name => node if local.any_k3s_compute }

  project_name                  = "${var.project_name}-worker-${each.key}"
  environment                   = var.network_environment
  instance_type                 = each.value.instance_type
  use_spot                      = each.value.use_spot
  subnet_id                     = local.networking.public_subnet_ids[0]
  vpc_id                        = local.networking.vpc_id
  vpc_cidr                      = var.network_vpc_cidr
  ssh_public_key                = var.ssh_public_key
  k3s_api_allowed_cidrs         = var.k3s_api_allowed_cidrs
  additional_security_group_ids = [local.networking.security_group_ids["indexer"]]
  tags                          = merge(local.common_tags, { "evm-cloud/role" = each.value.role })
  ingress_mode                  = var.ingress_mode
  secrets_mode                  = var.secrets_mode
  secrets_manager_prefix        = "evm-cloud/${var.project_name}"
  secrets_manager_secret_arn    = local.workload_secret_arn
}

module "k3s_agent" {
  source = "../../core/k8s/k3s-agent"
  count  = local.any_k3s_compute && length(var.k3s_worker_nodes) > 0 ? 1 : 0

  worker_nodes = [for node in var.k3s_worker_nodes : {
    name                 = node.name
    host                 = module.k3s_worker_host[node.name].host_ip
    ssh_user             = module.k3s_worker_host[node.name].ssh_user
    ssh_private_key_path = var.ssh_private_key_path
    ssh_port             = 22
    role                 = node.role
  }]

  server_host                 = module.k3s_host[0].host_ip
  server_ssh_user             = module.k3s_host[0].ssh_user
  server_ssh_private_key_path = var.ssh_private_key_path
  node_token                  = module.k3s_bootstrap[0].node_token
  k3s_version                 = var.k3s_version
  project_name                = var.project_name
}

# --- Secrets Management: SM secret for k3s/EKS provider mode ---

locals {
  # Should Terraform create the SM workload secret?
  create_workload_secret = (
    var.secrets_mode == "provider" &&
    var.secrets_manager_secret_arn == "" &&
    (local.any_k3s_compute || local.any_eks_compute)
  )

  # Resolved ARN: BYOA or Terraform-created
  workload_secret_arn = (
    var.secrets_manager_secret_arn != "" ? var.secrets_manager_secret_arn
    : local.create_workload_secret ? aws_secretsmanager_secret.workload[0].arn
    : ""
  )
}

resource "aws_secretsmanager_secret" "workload" {
  #checkov:skip=CKV2_AWS_57:Rotation configured externally by user
  count = local.create_workload_secret ? 1 : 0

  name                    = "evm-cloud/${var.project_name}/env"
  description             = "Workload secrets for evm-cloud project ${var.project_name}"
  kms_key_id              = var.secrets_manager_kms_key_id != "" ? var.secrets_manager_kms_key_id : null
  recovery_window_in_days = var.ec2_secret_recovery_window_in_days
}

resource "aws_secretsmanager_secret_version" "workload" {
  count = local.create_workload_secret ? 1 : 0

  secret_id = aws_secretsmanager_secret.workload[0].id
  secret_string = jsonencode(merge(
    var.indexer_storage_backend == "clickhouse" ? {
      CLICKHOUSE_URL      = var.indexer_clickhouse_url
      CLICKHOUSE_USER     = var.indexer_clickhouse_user
      CLICKHOUSE_PASSWORD = var.indexer_clickhouse_password
      CLICKHOUSE_DB       = var.indexer_clickhouse_db
    } : {},
    var.indexer_storage_backend == "postgres" && var.postgres_enabled ? {
      DATABASE_URL = local.eks_database_url
    } : {},
  ))
}

# --- Managed Postgres: credential resolution ---
# Used by EKS (K8s Secret), k3s inline secrets (handoff DATABASE_URL), and workload secret payload.

data "aws_secretsmanager_secret_version" "rds_master" {
  count     = (local._managed_pg_needs_url && var.postgres_manage_master_user_password) ? 1 : 0
  secret_id = module.postgres[0].master_secret_arn
}

locals {
  # Any engine with managed postgres that needs a constructed DATABASE_URL
  _managed_pg_needs_url = var.indexer_enabled && var.indexer_storage_backend == "postgres" && var.postgres_enabled && (
    (var.compute_engine == "eks" && local.terraform_manages_workloads) ||
    (var.compute_engine == "k3s") ||
    (var.compute_engine == "ec2")
  )
  _managed_pg_user = local._managed_pg_needs_url ? (
    !var.postgres_manage_master_user_password ? module.postgres[0].master_username : try(jsondecode(data.aws_secretsmanager_secret_version.rds_master[0].secret_string)["username"], "")
  ) : ""
  _managed_pg_pass = local._managed_pg_needs_url ? (
    !var.postgres_manage_master_user_password ? var.postgres_master_password : try(jsondecode(data.aws_secretsmanager_secret_version.rds_master[0].secret_string)["password"], "")
  ) : ""
  managed_database_url = local._managed_pg_needs_url ? (
    "postgresql://${local._managed_pg_user}:${urlencode(local._managed_pg_pass)}@${module.postgres[0].endpoint}:${module.postgres[0].port}/${module.postgres[0].db_name}"
  ) : ""

  # Backward compat alias
  eks_database_url = local.managed_database_url
}

# --- K8s Addons (Helm charts) ---

module "k8s_addons" {
  source = "../../core/k8s/addons"
  count  = (local.any_eks_compute && local.terraform_manages_workloads) ? 1 : 0

  providers = {
    kubernetes = kubernetes
    helm       = helm
  }

  project_name      = var.project_name
  eso_enabled       = var.secrets_mode != "inline"
  eso_chart_version = var.eso_chart_version

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
  aws_region                                     = var.aws_region
}

# --- RPC Proxy: eRPC (EKS) ---

module "eks_rpc_proxy" {
  source = "../../core/k8s/rpc-proxy"
  count  = (var.rpc_proxy_enabled && var.compute_engine == "eks" && local.terraform_manages_workloads) ? 1 : 0

  project_name       = var.project_name
  image              = var.rpc_proxy_image
  erpc_config_yaml   = var.erpc_config_yaml
  monitoring_enabled = var.monitoring_enabled
}

# --- Indexer: rindexer (EKS) ---

module "eks_indexer" {
  source = "../../core/k8s/indexer"
  count  = (var.indexer_enabled && var.compute_engine == "eks" && local.terraform_manages_workloads) ? 1 : 0

  project_name         = var.project_name
  image                = var.indexer_image
  rpc_url              = local.resolved_indexer_rpc_url
  rindexer_config_yaml = var.rindexer_config_yaml
  rindexer_abis        = var.rindexer_abis
  monitoring_enabled   = var.monitoring_enabled

  storage_backend = var.indexer_storage_backend

  # Postgres (pre-composed DATABASE_URL from RDS secret)
  database_url = local.eks_database_url

  # ClickHouse (BYODB)
  clickhouse_url      = var.indexer_clickhouse_url
  clickhouse_user     = var.indexer_clickhouse_user
  clickhouse_password = var.indexer_clickhouse_password
  clickhouse_db       = var.indexer_clickhouse_db
}
