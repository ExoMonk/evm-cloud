# Bare metal Docker Compose provisioning via SSH.
# Idempotent: setup triggers on host change, deploy triggers on config hash.

locals {
  # Render docker-compose.yml from shared template with local json-file logging
  docker_compose_content = templatefile("${path.module}/../../../core/docker-compose.yml.tpl", {
    enable_rpc_proxy    = var.enable_rpc_proxy
    enable_indexer      = var.enable_indexer
    rpc_proxy_image     = var.rpc_proxy_image
    indexer_image       = var.indexer_image
    rpc_proxy_mem_limit = var.rpc_proxy_mem_limit
    indexer_mem_limit   = var.indexer_mem_limit
    logging_driver      = "json-file"
    logging_options = {
      max-size = "50m"
      max-file = "5"
    }
  })

  # Render .env from secret payload
  env_content = join("\n", [for k, v in var.secret_payload : "${k}=${v}"])

  # Config content hash for deploy trigger
  config_hash = sha256(join("", [
    local.docker_compose_content,
    local.env_content,
    var.erpc_config_yaml,
    var.rindexer_config_yaml,
    jsonencode(var.rindexer_abis),
  ]))
}

# --- Phase 1: Host setup (Docker + directory structure) ---

resource "null_resource" "setup" {
  triggers = {
    host = var.host_address
  }

  connection {
    type        = "ssh"
    host        = var.host_address
    user        = var.ssh_user
    private_key = file(var.ssh_private_key_path)
    port        = var.ssh_port
  }

  provisioner "remote-exec" {
    inline = [templatefile("${path.module}/setup-host.sh.tpl", {
      project_name = var.project_name
    })]
  }
}

# --- Phase 2: Deploy configs + docker compose up ---

resource "null_resource" "deploy" {
  depends_on = [null_resource.setup]

  triggers = {
    config_hash     = local.config_hash
    host_address    = var.host_address
    ssh_user        = var.ssh_user
    ssh_private_key = var.ssh_private_key_path
    ssh_port        = var.ssh_port
  }

  connection {
    type        = "ssh"
    host        = self.triggers.host_address
    user        = self.triggers.ssh_user
    private_key = file(self.triggers.ssh_private_key)
    port        = self.triggers.ssh_port
  }

  # Upload docker-compose.yml
  provisioner "file" {
    content     = local.docker_compose_content
    destination = "/opt/evm-cloud/docker-compose.yml"
  }

  # Upload .env
  provisioner "file" {
    content     = local.env_content
    destination = "/opt/evm-cloud/.env"
  }

  # Upload erpc.yaml (if enabled)
  provisioner "file" {
    content     = var.erpc_config_yaml != "" ? var.erpc_config_yaml : "# erpc not enabled"
    destination = "/opt/evm-cloud/config/erpc.yaml"
  }

  # Upload rindexer.yaml (if enabled)
  provisioner "file" {
    content     = var.rindexer_config_yaml != "" ? var.rindexer_config_yaml : "# rindexer not enabled"
    destination = "/opt/evm-cloud/config/rindexer.yaml"
  }

  # Upload ABI files
  provisioner "file" {
    content     = jsonencode(var.rindexer_abis)
    destination = "/opt/evm-cloud/config/abis/_manifest.json"
  }

  # Deploy: extract ABIs from manifest + docker compose up
  provisioner "remote-exec" {
    inline = [templatefile("${path.module}/deploy.sh.tpl", {
      project_name = var.project_name
      abi_keys     = keys(var.rindexer_abis)
    })]
  }

  # Teardown on destroy: stop containers and remove config
  provisioner "remote-exec" {
    when = destroy
    inline = [
      "echo '[evm-cloud] Tearing down...'",
      "cd /opt/evm-cloud && docker compose down --remove-orphans 2>/dev/null || true",
      "rm -f /opt/evm-cloud/.env /opt/evm-cloud/docker-compose.yml",
      "echo '[evm-cloud] Teardown complete.'",
    ]
  }
}
