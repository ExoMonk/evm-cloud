module "networking" {
  source = "./networking"
  count  = var.networking_enabled ? 1 : 0

  project_name         = var.project_name
  environment          = var.network_environment
  vpc_cidr             = var.network_vpc_cidr
  availability_zones   = var.network_availability_zones
  enable_nat_gateway   = var.network_enable_nat_gateway
  enable_vpc_endpoints = var.network_enable_vpc_endpoints
}

locals {
  networking = var.networking_enabled ? {
    vpc_id             = module.networking[0].vpc_id
    public_subnet_ids  = module.networking[0].public_subnet_ids
    private_subnet_ids = module.networking[0].private_subnet_ids
    security_group_ids = module.networking[0].security_group_ids
  } : null

  any_compute_enabled         = var.rpc_proxy_enabled || var.indexer_enabled
  any_ecs_compute             = local.any_compute_enabled && var.compute_engine == "ecs"
  any_eks_compute             = local.any_compute_enabled && var.compute_engine == "eks"
  terraform_manages_workloads = var.workload_mode == "terraform"

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

# --- Shared ECS Cluster ---

module "ecs_cluster" {
  #checkov:skip=CKV_TF_1:Registry version pins are standard for community modules
  source  = "terraform-aws-modules/ecs/aws//modules/cluster"
  version = "~> 6.0"
  count   = local.any_ecs_compute ? 1 : 0

  name = "${var.project_name}-${var.network_environment}"

  # Container Insights enabled by default in module (CKV_AWS_65)

  # Use FARGATE as default capacity provider
  default_capacity_provider_strategy = {
    FARGATE = {
      base   = 1
      weight = 100
    }
  }

  tags = local.common_tags
}

# --- ECS Service Discovery (Cloud Map) ---

resource "aws_service_discovery_private_dns_namespace" "ecs" {
  count = local.any_ecs_compute ? 1 : 0

  name = "${var.project_name}-${var.network_environment}.internal"
  vpc  = local.networking.vpc_id

  tags = local.common_tags
}

resource "aws_service_discovery_service" "rpc_proxy" {
  count = (local.any_ecs_compute && var.rpc_proxy_enabled) ? 1 : 0

  name = "${var.project_name}-erpc"

  dns_config {
    namespace_id = aws_service_discovery_private_dns_namespace.ecs[0].id

    dns_records {
      ttl  = 10
      type = "A"
    }

    routing_policy = "MULTIVALUE"
  }

  tags = local.common_tags
}

# --- Database: PostgreSQL ---

module "postgres" {
  source = "./database/postgres"
  count  = var.postgres_enabled ? 1 : 0

  project_name      = var.project_name
  environment       = var.network_environment
  subnet_ids        = local.networking.private_subnet_ids
  security_group_id = local.networking.security_group_ids["database"]

  engine_version          = var.postgres_engine_version
  instance_class          = var.postgres_instance_class
  db_name                 = var.postgres_db_name
  db_username             = var.postgres_db_username
  backup_retention_period = var.postgres_backup_retention
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

# --- EKS: Postgres secret resolution ---
# EKS indexer needs a pre-composed DATABASE_URL. Extract RDS password from
# Secrets Manager at plan time to build it.

data "aws_secretsmanager_secret_version" "rds_master" {
  count     = (var.indexer_enabled && var.compute_engine == "eks" && var.indexer_storage_backend == "postgres" && local.terraform_manages_workloads) ? 1 : 0
  secret_id = module.postgres[0].master_secret_arn
}

locals {
  eks_database_url = (var.indexer_enabled && var.compute_engine == "eks" && var.indexer_storage_backend == "postgres" && local.terraform_manages_workloads) ? (
    "postgresql://${jsondecode(data.aws_secretsmanager_secret_version.rds_master[0].secret_string)["username"]}:${jsondecode(data.aws_secretsmanager_secret_version.rds_master[0].secret_string)["password"]}@${module.postgres[0].endpoint}:${module.postgres[0].port}/${module.postgres[0].db_name}"
  ) : ""
}

# --- RPC Proxy: eRPC (ECS) ---

module "rpc_proxy" {
  source = "./ecs/rpc-proxy"
  count  = (var.rpc_proxy_enabled && var.compute_engine == "ecs" && local.terraform_manages_workloads) ? 1 : 0

  project_name      = var.project_name
  environment       = var.network_environment
  subnet_ids        = local.networking.private_subnet_ids
  security_group_id = local.networking.security_group_ids["erpc"]
  cluster_arn       = module.ecs_cluster[0].arn
  image             = var.rpc_proxy_image
  aws_region        = var.aws_region

  task_execution_role_arn = aws_iam_role.ecs_task_execution[0].arn
  task_role_arn           = aws_iam_role.ecs_task_rpc_proxy[0].arn

  service_discovery_service_arn = aws_service_discovery_service.rpc_proxy[0].arn

  config_bucket_name = aws_s3_bucket.config[0].id
  config_object_key  = aws_s3_object.erpc_config[0].key
}

# --- RPC Proxy: eRPC (EKS) ---

module "eks_rpc_proxy" {
  source = "./eks/rpc-proxy"
  count  = (var.rpc_proxy_enabled && var.compute_engine == "eks" && local.terraform_manages_workloads) ? 1 : 0

  project_name     = var.project_name
  image            = var.rpc_proxy_image
  erpc_config_yaml = var.erpc_config_yaml
}

# --- Indexer: rindexer (ECS) ---

module "indexer" {
  source = "./ecs/indexer"
  count  = (var.indexer_enabled && var.compute_engine == "ecs" && local.terraform_manages_workloads) ? 1 : 0

  project_name      = var.project_name
  environment       = var.network_environment
  subnet_ids        = local.networking.private_subnet_ids
  security_group_id = local.networking.security_group_ids["indexer"]
  cluster_arn       = module.ecs_cluster[0].arn
  image             = var.indexer_image
  rpc_url           = var.indexer_rpc_url
  aws_region        = var.aws_region

  task_execution_role_arn = aws_iam_role.ecs_task_execution[0].arn
  task_role_arn           = aws_iam_role.ecs_task_indexer[0].arn

  storage_backend = var.indexer_storage_backend

  # Postgres (from managed RDS — only used when storage_backend=postgres)
  db_secret_arn = var.indexer_storage_backend == "postgres" ? module.postgres[0].master_secret_arn : ""
  db_host       = var.indexer_storage_backend == "postgres" ? module.postgres[0].endpoint : ""
  db_port       = var.indexer_storage_backend == "postgres" ? module.postgres[0].port : 5432
  db_name       = var.indexer_storage_backend == "postgres" ? module.postgres[0].db_name : ""

  # ClickHouse (BYODB — only used when storage_backend=clickhouse)
  clickhouse_url      = var.indexer_clickhouse_url
  clickhouse_user     = var.indexer_clickhouse_user
  clickhouse_password = var.indexer_clickhouse_password
  clickhouse_db       = var.indexer_clickhouse_db

  config_bucket_name   = aws_s3_bucket.config[0].id
  config_object_prefix = "rindexer"
}

# --- Indexer: rindexer (EKS) ---

module "eks_indexer" {
  source = "./eks/indexer"
  count  = (var.indexer_enabled && var.compute_engine == "eks" && local.terraform_manages_workloads) ? 1 : 0

  project_name         = var.project_name
  image                = var.indexer_image
  rpc_url              = var.indexer_rpc_url
  rindexer_config_yaml = var.rindexer_config_yaml
  rindexer_abis        = var.rindexer_abis

  storage_backend = var.indexer_storage_backend

  # Postgres (pre-composed DATABASE_URL from RDS secret)
  database_url = local.eks_database_url

  # ClickHouse (BYODB)
  clickhouse_url      = var.indexer_clickhouse_url
  clickhouse_user     = var.indexer_clickhouse_user
  clickhouse_password = var.indexer_clickhouse_password
  clickhouse_db       = var.indexer_clickhouse_db
}
