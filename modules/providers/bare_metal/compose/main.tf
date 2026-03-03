# Bare metal Docker Compose provisioning via SSH.
# Idempotent: setup triggers on host change, deploy triggers on config hash.

locals {
  # Caddy is enabled for cloudflare and caddy ingress modes
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

  # Render docker-compose.yml from shared template with local json-file logging
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
    local.caddyfile_content,
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

  # Harden .env and cert file permissions in cloudflare mode; clean cert artifacts in other modes
  provisioner "remote-exec" {
    inline = [
      "chmod 0600 /opt/evm-cloud/.env",
      "if [ '${var.ingress_mode}' = 'cloudflare' ]; then chmod 0600 /opt/evm-cloud/certs/*.pem 2>/dev/null || true; else rm -f /opt/evm-cloud/certs/origin.pem /opt/evm-cloud/certs/origin-key.pem; fi",
    ]
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
