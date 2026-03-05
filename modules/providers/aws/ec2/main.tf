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

  # Caddy is enabled for cloudflare and caddy ingress modes on EC2
  enable_caddy = contains(["cloudflare", "caddy"], var.ingress_mode)

  # Render Caddyfile from shared template (only when caddy is enabled)
  caddyfile_content = local.enable_caddy ? templatefile("${path.module}/../../../core/templates/Caddyfile.tpl", {
    ingress_mode          = var.ingress_mode
    domain                = var.erpc_hostname
    tls_email             = var.ingress_tls_email
    tls_staging           = var.ingress_tls_staging
    hsts_preload          = var.ingress_hsts_preload
    request_body_max_size = var.ingress_request_body_max_size
    enable_rpc_proxy      = var.enable_rpc_proxy
  }) : ""

  # Cloudflare mode: mount origin cert files into Caddy container
  caddy_cert_volumes = var.ingress_mode == "cloudflare" ? join("\n      ", [
    "- /opt/evm-cloud/certs/origin.pem:/etc/caddy/certs/origin.pem:ro",
    "- /opt/evm-cloud/certs/origin-key.pem:/etc/caddy/certs/origin-key.pem:ro",
  ]) : ""

  # Render docker-compose.yml from shared template with AWS logging
  docker_compose_content = templatefile("${path.module}/../../../core/docker-compose.yml.tpl", {
    enable_rpc_proxy    = var.enable_rpc_proxy
    enable_indexer      = var.enable_indexer
    enable_caddy        = local.enable_caddy
    rpc_proxy_image     = var.rpc_proxy_image
    indexer_image       = var.indexer_image
    caddy_image         = var.ingress_caddy_image
    rpc_proxy_mem_limit = var.rpc_proxy_mem_limit
    indexer_mem_limit   = var.indexer_mem_limit
    caddy_mem_limit     = var.ingress_caddy_mem_limit
    caddy_cert_volumes  = local.caddy_cert_volumes
    logging_driver      = "awslogs"
    logging_options = {
      awslogs-region = var.aws_region
      awslogs-group  = aws_cloudwatch_log_group.services.name
      awslogs-stream = "evm-cloud"
    }
  })

  # Config content hash for deploy trigger (mirrors bare_metal compose pattern)
  config_hash = sha256(join("", [
    local.docker_compose_content,
    local.caddyfile_content,
    var.erpc_yaml_content,
    var.rindexer_yaml_content,
    jsonencode(var.abi_files),
  ]))

  # Render cloud-init from template
  cloud_init_content = templatefile("${path.module}/cloud-init.yml.tpl", {
    workload_mode          = var.workload_mode
    docker_compose_content = local.docker_compose_content
    enable_rpc_proxy       = var.enable_rpc_proxy
    enable_indexer         = var.enable_indexer
    enable_caddy           = local.enable_caddy
    caddyfile_content      = local.caddyfile_content
    cloudflare_origin_cert = var.ingress_cloudflare_origin_cert
    cloudflare_origin_key  = var.ingress_cloudflare_origin_key
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

# --- Config Update (terraform mode only) ---
# Mirrors bare_metal compose pattern: triggers on config hash change,
# re-uploads configs via SSH, force-recreates containers.

resource "null_resource" "config_update" {
  count      = var.workload_mode == "terraform" && var.ssh_private_key_path != "" ? 1 : 0
  depends_on = [aws_instance.this]

  triggers = {
    config_hash     = local.config_hash
    instance_ip     = aws_instance.this.public_ip
    ssh_user        = var.ssh_user
    ssh_private_key = var.ssh_private_key_path
  }

  connection {
    type        = "ssh"
    host        = self.triggers.instance_ip
    user        = self.triggers.ssh_user
    private_key = file(pathexpand(self.triggers.ssh_private_key))
    port        = 22
  }

  # Wait for cloud-init to finish and ensure directory ownership.
  # cloud-init writes files as root, then runcmd creates dirs + chowns.
  # We must wait for the full cycle before uploading via SCP.
  provisioner "remote-exec" {
    inline = [
      "sudo cloud-init status --wait >/dev/null 2>&1 || true",
      "sudo mkdir -p /opt/evm-cloud/config/abis /opt/evm-cloud/scripts /opt/evm-cloud/certs",
      "sudo chown -R ${self.triggers.ssh_user}:${self.triggers.ssh_user} /opt/evm-cloud",
    ]
  }

  # Upload docker-compose.yml
  provisioner "file" {
    content     = local.docker_compose_content
    destination = "/opt/evm-cloud/docker-compose.yml"
  }

  # Upload erpc.yaml (if enabled)
  provisioner "file" {
    content     = var.erpc_yaml_content != "" ? var.erpc_yaml_content : "# erpc not enabled"
    destination = "/opt/evm-cloud/config/erpc.yaml"
  }

  # Upload rindexer.yaml (if enabled)
  provisioner "file" {
    content     = var.rindexer_yaml_content != "" ? var.rindexer_yaml_content : "# rindexer not enabled"
    destination = "/opt/evm-cloud/config/rindexer.yaml"
  }

  # Upload ABI manifest
  provisioner "file" {
    content     = jsonencode(var.abi_files)
    destination = "/opt/evm-cloud/config/abis/_manifest.json"
  }

  # Upload Caddyfile (if ingress enabled)
  provisioner "file" {
    content     = local.caddyfile_content != "" ? local.caddyfile_content : "# caddy not enabled"
    destination = "/opt/evm-cloud/config/Caddyfile"
  }

  # Upload Cloudflare origin cert (if cloudflare mode)
  provisioner "file" {
    content     = var.ingress_mode == "cloudflare" ? var.ingress_cloudflare_origin_cert : ""
    destination = "/opt/evm-cloud/certs/origin.pem"
  }

  # Upload Cloudflare origin key (if cloudflare mode)
  provisioner "file" {
    content     = var.ingress_mode == "cloudflare" ? var.ingress_cloudflare_origin_key : ""
    destination = "/opt/evm-cloud/certs/origin-key.pem"
  }

  # Harden cert file permissions in cloudflare mode; clean cert artifacts in other modes
  provisioner "remote-exec" {
    inline = [
      "if [ '${var.ingress_mode}' = 'cloudflare' ]; then chmod 0600 /opt/evm-cloud/certs/*.pem; else rm -f /opt/evm-cloud/certs/origin.pem /opt/evm-cloud/certs/origin-key.pem; fi",
    ]
  }

  # Deploy: refresh secrets, extract ABIs, force-recreate containers
  provisioner "remote-exec" {
    inline = [templatefile("${path.module}/deploy.sh.tpl", {
      project_name = var.project_name
    })]
  }
}
