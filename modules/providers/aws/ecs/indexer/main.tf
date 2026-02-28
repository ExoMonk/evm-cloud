locals {
  common_tags = {
    Project     = var.project_name
    Environment = var.environment
    ManagedBy   = "terraform"
    Module      = "indexer"
  }

  is_postgres = var.storage_backend == "postgres"

  # S3 config pull commands (shared across backends)
  s3_pull_commands = [
    "mkdir -p /tmp/project/abis",
    "aws s3 cp s3://${var.config_bucket_name}/${var.config_object_prefix}/rindexer.yaml /tmp/project/rindexer.yaml",
    "aws s3 sync s3://${var.config_bucket_name}/${var.config_object_prefix}/abis/ /tmp/project/abis/",
  ]

  # Postgres: compose DATABASE_URL from Secrets Manager creds at runtime
  postgres_entrypoint = join(" && ", concat(
    local.s3_pull_commands,
    [
      "export DATABASE_URL=\"postgresql://$${DATABASE_USER}:$${DATABASE_PASSWORD}@$${DATABASE_HOST}:$${DATABASE_PORT}/$${DATABASE_NAME}\"",
      "exec rindexer start --path /tmp/project"
    ]
  ))

  # ClickHouse: env vars are injected directly, just pull config and start
  clickhouse_entrypoint = join(" && ", concat(
    local.s3_pull_commands,
    ["exec rindexer start --path /tmp/project"]
  ))

  # Environment variables per backend
  common_env = [
    { name = "RPC_URL", value = var.rpc_url },
    { name = "AWS_DEFAULT_REGION", value = var.aws_region },
  ]

  postgres_env = [
    { name = "DATABASE_HOST", value = var.db_host },
    { name = "DATABASE_PORT", value = tostring(var.db_port) },
    { name = "DATABASE_NAME", value = var.db_name },
  ]

  clickhouse_env = [
    { name = "CLICKHOUSE_URL", value = var.clickhouse_url },
    { name = "CLICKHOUSE_USER", value = var.clickhouse_user },
    { name = "CLICKHOUSE_PASSWORD", value = var.clickhouse_password },
    { name = "CLICKHOUSE_DB", value = var.clickhouse_db },
  ]

  # Secrets (Postgres only — ClickHouse uses plain env vars for BYODB)
  postgres_secrets = [
    {
      name      = "DATABASE_USER"
      valueFrom = "${var.db_secret_arn}:username::"
    },
    {
      name      = "DATABASE_PASSWORD"
      valueFrom = "${var.db_secret_arn}:password::"
    }
  ]
}

resource "aws_cloudwatch_log_group" "this" {
  #checkov:skip=CKV_AWS_338:Tier 0 uses 30-day log retention
  #checkov:skip=CKV_AWS_158:KMS encryption optional for Tier 0
  name              = "/ecs/${var.project_name}/${var.environment}/indexer"
  retention_in_days = 30
  tags              = merge(local.common_tags, { Name = "${var.project_name}-${var.environment}-indexer-logs" })
}

# Single-writer constraint: rindexer must run exactly one active writer per
# project dataset. desired_count is hardcoded to 1 and autoscaling is omitted.
# Scaling the indexer requires partitioning by contract/event, not replicas.
module "ecs_service" {
  #checkov:skip=CKV_TF_1:Registry version pins are standard for community modules
  source  = "terraform-aws-modules/ecs/aws//modules/service"
  version = "~> 6.0"

  name        = "${var.project_name}-indexer"
  cluster_arn = var.cluster_arn
  cpu         = var.cpu
  memory      = var.memory

  desired_count = 1

  # Fargate launch type
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"

  container_definitions = {
    indexer = {
      cpu       = var.cpu
      memory    = var.memory
      essential = true
      image     = var.image

      # Fargate has writable /tmp by default; S3 pull writes config there.
      readonlyRootFilesystem = true

      command = [
        "sh", "-c",
        local.is_postgres ? local.postgres_entrypoint : local.clickhouse_entrypoint
      ]

      environment = concat(
        local.common_env,
        local.is_postgres ? local.postgres_env : local.clickhouse_env
      )

      secrets = local.is_postgres ? local.postgres_secrets : []

      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = aws_cloudwatch_log_group.this.name
          "awslogs-region"        = var.aws_region
          "awslogs-stream-prefix" = "indexer"
        }
      }
    }
  }

  # Grant S3 read access for config pull
  tasks_iam_role_statements = [
    {
      effect  = "Allow"
      actions = ["s3:GetObject", "s3:ListBucket"]
      resources = [
        "arn:aws:s3:::${var.config_bucket_name}",
        "arn:aws:s3:::${var.config_bucket_name}/${var.config_object_prefix}/*"
      ]
    }
  ]

  subnet_ids            = var.subnet_ids
  create_security_group = false
  security_group_ids    = [var.security_group_id]

  tags = local.common_tags
}
