locals {
  common_tags = {
    Project     = var.project_name
    Environment = var.environment
    ManagedBy   = "terraform"
    Module      = "rpc-proxy"
  }
}

resource "aws_cloudwatch_log_group" "this" {
  #checkov:skip=CKV_AWS_338:Tier 0 uses 30-day log retention
  #checkov:skip=CKV_AWS_158:KMS encryption optional for Tier 0
  name              = "/ecs/${var.project_name}/${var.environment}/erpc"
  retention_in_days = 30
  tags              = merge(local.common_tags, { Name = "${var.project_name}-${var.environment}-erpc-logs" })
}

module "ecs_service" {
  #checkov:skip=CKV_TF_1:Registry version pins are standard for community modules
  source  = "terraform-aws-modules/ecs/aws//modules/service"
  version = "~> 6.0"

  name        = "${var.project_name}-erpc"
  cluster_arn = var.cluster_arn
  cpu         = var.cpu
  memory      = var.memory

  # Fargate launch type
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"

  create_task_exec_iam_role = false
  task_exec_iam_role_arn    = var.task_execution_role_arn
  create_tasks_iam_role     = false
  tasks_iam_role_arn        = var.task_role_arn

  service_registries = var.service_discovery_service_arn != "" ? {
    registry_arn = var.service_discovery_service_arn
    port         = var.container_port
  } : {}

  container_definitions = {
    erpc = {
      cpu       = var.cpu
      memory    = var.memory
      essential = true
      image     = var.image

      # Fargate has writable /tmp by default; S3 pull writes config there.
      readonlyRootFilesystem = true

      portMappings = [
        {
          containerPort = var.container_port
          hostPort      = var.container_port
          protocol      = "tcp"
        }
      ]

      # eRPC reads erpc.yaml — NOT env vars. Pull config from S3 at boot.
      command = [
        "sh", "-c",
        "aws s3 cp s3://${var.config_bucket_name}/${var.config_object_key} /tmp/erpc.yaml && exec erpc-server --config /tmp/erpc.yaml"
      ]

      environment = [
        { name = "AWS_DEFAULT_REGION", value = var.aws_region },
      ]

      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = aws_cloudwatch_log_group.this.name
          "awslogs-region"        = var.aws_region
          "awslogs-stream-prefix" = "erpc"
        }
      }
    }
  }

  subnet_ids            = var.subnet_ids
  create_security_group = false
  security_group_ids    = [var.security_group_id]

  tags = local.common_tags
}
