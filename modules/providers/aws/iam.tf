locals {
  ecs_needs_rpc_role     = local.any_ecs_compute && var.rpc_proxy_enabled
  ecs_needs_indexer_role = local.any_ecs_compute && var.indexer_enabled
}

data "aws_iam_policy_document" "ecs_task_assume_role" {
  statement {
    effect = "Allow"
    principals {
      type        = "Service"
      identifiers = ["ecs-tasks.amazonaws.com"]
    }
    actions = ["sts:AssumeRole"]
  }
}

# Shared ECS task execution role (ECR pull, CloudWatch logs, ECS-managed integrations)
resource "aws_iam_role" "ecs_task_execution" {
  count = local.any_ecs_compute ? 1 : 0

  name               = "${var.project_name}-${var.network_environment}-ecs-task-exec"
  assume_role_policy = data.aws_iam_policy_document.ecs_task_assume_role.json
  tags               = local.common_tags
}

resource "aws_iam_role_policy_attachment" "ecs_task_execution_managed" {
  count = local.any_ecs_compute ? 1 : 0

  role       = aws_iam_role.ecs_task_execution[0].name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy"
}

# Allow ECS execution role to resolve Postgres credentials from Secrets Manager
resource "aws_iam_role_policy" "ecs_task_execution_secrets" {
  count = (local.any_ecs_compute && var.indexer_enabled && var.indexer_storage_backend == "postgres") ? 1 : 0

  name = "${var.project_name}-${var.network_environment}-ecs-task-exec-secrets"
  role = aws_iam_role.ecs_task_execution[0].id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "secretsmanager:GetSecretValue"
        ]
        Resource = module.postgres[0].master_secret_arn
      }
    ]
  })
}

# Allow ECS execution role to resolve external ClickHouse password secret
resource "aws_iam_role_policy" "ecs_task_execution_clickhouse_secret" {
  count = (local.any_ecs_compute && var.indexer_enabled && var.indexer_storage_backend == "clickhouse") ? 1 : 0

  name = "${var.project_name}-${var.network_environment}-ecs-task-exec-clickhouse-secret"
  role = aws_iam_role.ecs_task_execution[0].id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "secretsmanager:GetSecretValue"
        ]
        Resource = "arn:aws:secretsmanager:${var.aws_region}:*:secret:evm-cloud/indexer/clickhouse-password*"
      }
    ]
  })
}

resource "aws_iam_role" "ecs_task_rpc_proxy" {
  count = local.ecs_needs_rpc_role ? 1 : 0

  name               = "${var.project_name}-${var.network_environment}-ecs-task-rpc-proxy"
  assume_role_policy = data.aws_iam_policy_document.ecs_task_assume_role.json
  tags               = local.common_tags
}

resource "aws_iam_role_policy" "ecs_task_rpc_proxy_s3" {
  count = local.ecs_needs_rpc_role ? 1 : 0

  name = "${var.project_name}-${var.network_environment}-ecs-task-rpc-proxy-s3"
  role = aws_iam_role.ecs_task_rpc_proxy[0].id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "s3:GetObject"
        ]
        Resource = "arn:aws:s3:::${aws_s3_bucket.config[0].id}/erpc/*"
      }
    ]
  })
}

resource "aws_iam_role" "ecs_task_indexer" {
  count = local.ecs_needs_indexer_role ? 1 : 0

  name               = "${var.project_name}-${var.network_environment}-ecs-task-indexer"
  assume_role_policy = data.aws_iam_policy_document.ecs_task_assume_role.json
  tags               = local.common_tags
}

resource "aws_iam_role_policy" "ecs_task_indexer_s3" {
  count = local.ecs_needs_indexer_role ? 1 : 0

  name = "${var.project_name}-${var.network_environment}-ecs-task-indexer-s3"
  role = aws_iam_role.ecs_task_indexer[0].id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "s3:ListBucket"
        ]
        Resource = "arn:aws:s3:::${aws_s3_bucket.config[0].id}"
      },
      {
        Effect = "Allow"
        Action = [
          "s3:GetObject"
        ]
        Resource = "arn:aws:s3:::${aws_s3_bucket.config[0].id}/rindexer/*"
      }
    ]
  })
}