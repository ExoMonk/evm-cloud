# EC2+Docker Compose compute engine — single instance running eRPC + rindexer
# as separate containers via Docker Compose with bind-mounted config files.

locals {
  secret_id = "${var.secret_name_prefix}/${var.project_name}/env"

  # Build secret JSON payload from conditional values
  secret_payload = merge(
    var.rpc_url != "" ? { RPC_URL = var.rpc_url } : {},
    var.storage_backend == "postgres" ? {
      DATABASE_URL = "postgresql://${var.db_username}:${urlencode(var.db_password)}@${var.db_host}:${var.db_port}/${var.db_name}"
    } : {},
    var.storage_backend == "clickhouse" ? {
      CLICKHOUSE_URL      = var.clickhouse_url
      CLICKHOUSE_USER     = var.clickhouse_user
      CLICKHOUSE_PASSWORD = var.clickhouse_password
      CLICKHOUSE_DB       = var.clickhouse_db
    } : {},
  )

  # Render docker-compose.yml from shared template with AWS logging
  docker_compose_content = templatefile("${path.module}/../../../core/docker-compose.yml.tpl", {
    enable_rpc_proxy    = var.enable_rpc_proxy
    enable_indexer      = var.enable_indexer
    rpc_proxy_image     = var.rpc_proxy_image
    indexer_image       = var.indexer_image
    rpc_proxy_mem_limit = var.rpc_proxy_mem_limit
    indexer_mem_limit   = var.indexer_mem_limit
    logging_driver      = "awslogs"
    logging_options = {
      awslogs-region = var.aws_region
      awslogs-group  = aws_cloudwatch_log_group.services.name
      awslogs-stream = "evm-cloud"
    }
  })

  # Render cloud-init from template
  cloud_init_content = templatefile("${path.module}/cloud-init.yml.tpl", {
    workload_mode          = var.workload_mode
    docker_compose_content = local.docker_compose_content
    enable_rpc_proxy       = var.enable_rpc_proxy
    enable_indexer         = var.enable_indexer
    erpc_yaml_content      = var.erpc_yaml_content
    rindexer_yaml_content  = var.rindexer_yaml_content
    abi_files              = var.abi_files
    pull_secrets_script = templatefile("${path.module}/pull-secrets.sh.tpl", {
      secret_id  = local.secret_id
      aws_region = var.aws_region
    })
  })
}

# --- AMI ---

data "aws_ami" "al2023" {
  most_recent = true
  owners      = ["amazon"]

  filter {
    name   = "name"
    values = ["al2023-ami-*-x86_64"]
  }

  filter {
    name   = "virtualization-type"
    values = ["hvm"]
  }
}

# --- SSH Key Pair ---

resource "aws_key_pair" "deploy" {
  key_name   = "${var.project_name}-${var.environment}-deploy"
  public_key = var.ssh_public_key
  tags       = var.tags
}

# --- CloudWatch Log Group ---

resource "aws_cloudwatch_log_group" "services" {
  #checkov:skip=CKV_AWS_158:KMS encryption optional for dev-tier log group
  #checkov:skip=CKV_AWS_338:30-day retention is sufficient for dev-tier indexer logs
  name              = "/evm-cloud/${var.project_name}-${var.environment}"
  retention_in_days = 30
  tags              = var.tags
}

# --- Secrets Manager ---

resource "aws_secretsmanager_secret" "env" {
  #checkov:skip=CKV_AWS_149:KMS encryption optional for Tier 0
  name                    = local.secret_id
  recovery_window_in_days = var.secret_recovery_window_in_days
  tags                    = var.tags
}

resource "aws_secretsmanager_secret_version" "env" {
  secret_id     = aws_secretsmanager_secret.env.id
  secret_string = jsonencode(local.secret_payload)
}

# --- EC2 Instance ---

resource "aws_instance" "this" {
  #checkov:skip=CKV_AWS_88:Public IP needed for dev SSH access
  #checkov:skip=CKV_AWS_8:Launch config not used — direct EC2 instance
  #checkov:skip=CKV2_AWS_41:IAM instance profile attached via instance_profile_name
  #checkov:skip=CKV_AWS_126:Detailed monitoring not needed for dev-tier single instance
  #checkov:skip=CKV_AWS_135:EBS optimization automatic for t3+ instances
  ami                    = data.aws_ami.al2023.id
  instance_type          = var.instance_type
  subnet_id              = var.subnet_id
  vpc_security_group_ids = concat([var.security_group_id], var.additional_security_group_ids)
  iam_instance_profile   = var.instance_profile_name
  key_name               = aws_key_pair.deploy.key_name

  associate_public_ip_address = true

  metadata_options {
    http_endpoint = "enabled"
    http_tokens   = "required"
  }

  root_block_device {
    volume_size           = var.root_volume_size
    volume_type           = "gp3"
    encrypted             = true
    delete_on_termination = true
  }

  user_data = local.cloud_init_content

  lifecycle {
    ignore_changes = [user_data]

    precondition {
      condition     = length(local.cloud_init_content) <= 16384
      error_message = "Cloud-init payload exceeds 16KB user_data limit. Reduce config/ABI size or use workload_mode=external."
    }
  }

  tags = merge(var.tags, {
    Name = "${var.project_name}-${var.environment}"
  })
}
