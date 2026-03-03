locals {
  common_tags = {
    Project     = var.project_name
    Environment = var.environment
    ManagedBy   = "terraform"
    Module      = "database/postgres"
  }

  use_explicit_password = !var.manage_master_user_password
}

resource "aws_db_subnet_group" "this" {
  name       = "${var.project_name}-${var.environment}-postgres"
  subnet_ids = var.subnet_ids
  tags       = merge(local.common_tags, { Name = "${var.project_name}-${var.environment}-postgres-subnet-group" })
}

module "rds" {
  #checkov:skip=CKV_AWS_157:Tier 0 single-instance; multi_az configurable via variable
  #checkov:skip=CKV_AWS_161:IAM auth not required for Tier 0; Secrets Manager password used
  #checkov:skip=CKV_TF_1:Registry version pins are standard for community modules
  source  = "terraform-aws-modules/rds/aws"
  version = "~> 6.0"

  identifier           = "${var.project_name}-${var.environment}"
  engine               = "postgres"
  engine_version       = var.engine_version
  family               = "postgres16"
  major_engine_version = "16"
  instance_class       = var.instance_class

  allocated_storage     = var.allocated_storage
  max_allocated_storage = var.max_allocated_storage

  db_name  = var.db_name
  username = var.db_username
  port     = 5432

  # Explicit password: user manages the secret externally.
  # Default: AWS manages the password via Secrets Manager (automatic rotation).
  manage_master_user_password = !local.use_explicit_password
  password                    = var.master_password

  multi_az               = var.multi_az
  db_subnet_group_name   = aws_db_subnet_group.this.name
  vpc_security_group_ids = [var.security_group_id]
  publicly_accessible    = false
  storage_encrypted      = true

  backup_retention_period = var.backup_retention_period
  skip_final_snapshot     = var.environment != "production"
  deletion_protection     = var.deletion_protection != null ? var.deletion_protection : var.environment == "production"

  monitoring_interval          = 60
  create_monitoring_role       = true
  performance_insights_enabled = true

  enabled_cloudwatch_logs_exports = ["postgresql", "upgrade"]
  create_cloudwatch_log_group     = true

  parameters = [
    { name = "log_connections", value = "1" },
    { name = "log_disconnections", value = "1" },
    { name = "rds.force_ssl", value = "0" }
  ]

  tags = local.common_tags
}
