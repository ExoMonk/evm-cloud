# Bare Metal / VPS provider adapter — deploys rindexer + eRPC on any VPS via SSH.
# Supports docker_compose and k3s compute engines.

locals {
  terraform_manages_workloads = var.workload_mode == "terraform"
  any_compute_enabled         = var.rpc_proxy_enabled || var.indexer_enabled

  # Auto-wire indexer → eRPC when both are enabled and user didn't provide explicit URL.
  erpc_internal_url        = var.rpc_proxy_enabled ? "http://erpc:4000" : ""
  resolved_indexer_rpc_url = var.indexer_rpc_url != "" ? var.indexer_rpc_url : local.erpc_internal_url

  # Secret payload — same shape as EC2 module for consistency
  secret_payload = merge(
    local.resolved_indexer_rpc_url != "" ? { RPC_URL = local.resolved_indexer_rpc_url } : {},
    var.indexer_storage_backend == "postgres" && var.indexer_postgres_url != "" ? {
      DATABASE_URL = var.indexer_postgres_url
    } : {},
    var.indexer_storage_backend == "clickhouse" ? {
      CLICKHOUSE_URL      = var.indexer_clickhouse_url
      CLICKHOUSE_USER     = var.indexer_clickhouse_user
      CLICKHOUSE_PASSWORD = var.indexer_clickhouse_password
      CLICKHOUSE_DB       = var.indexer_clickhouse_db
    } : {},
    var.indexer_extra_env,
    var.indexer_extra_secret_env,
  )
}

# --- Guardrails ---

resource "terraform_data" "config_required" {
  count = (local.any_compute_enabled && local.terraform_manages_workloads) ? 1 : 0

  lifecycle {
    precondition {
      condition     = !(var.rpc_proxy_enabled && var.erpc_config_yaml == "")
      error_message = "erpc_config_yaml is required when rpc_proxy_enabled=true."
    }

    precondition {
      condition     = !(var.indexer_enabled && var.rindexer_config_yaml == "")
      error_message = "rindexer_config_yaml is required when indexer_enabled=true."
    }

    precondition {
      condition     = !(var.indexer_enabled && var.indexer_storage_backend == "clickhouse" && var.indexer_clickhouse_url == "")
      error_message = "indexer_clickhouse_url is required when indexer_storage_backend=clickhouse."
    }
  }
}

# --- Docker Compose mode ---

module "compose" {
  source = "./compose"
  count  = (var.compute_engine == "docker_compose" && local.terraform_manages_workloads) ? 1 : 0

  project_name         = var.project_name
  host_address         = var.host_address
  ssh_user             = var.ssh_user
  ssh_private_key_path = var.ssh_private_key_path
  ssh_port             = var.ssh_port

  enable_rpc_proxy    = var.rpc_proxy_enabled
  enable_indexer      = var.indexer_enabled
  rpc_proxy_image     = var.rpc_proxy_image
  indexer_image       = var.indexer_image
  rpc_proxy_mem_limit = var.rpc_proxy_mem_limit
  indexer_mem_limit   = var.indexer_mem_limit

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

  erpc_config_yaml     = var.erpc_config_yaml
  rindexer_config_yaml = var.rindexer_config_yaml
  rindexer_abis        = var.rindexer_abis
  secret_payload       = local.secret_payload
}

# --- k3s mode ---

module "k3s_bootstrap" {
  source = "../../core/k8s/k3s-bootstrap"
  count  = var.compute_engine == "k3s" ? 1 : 0

  host_address         = var.host_address
  ssh_user             = var.ssh_user
  ssh_private_key_path = var.ssh_private_key_path
  ssh_port             = var.ssh_port
  project_name         = var.project_name
  k3s_version          = var.k3s_version
  tls_san_entries      = [var.host_address]
}

# --- k3s worker nodes ---

module "k3s_agent" {
  source = "../../core/k8s/k3s-agent"
  count  = var.compute_engine == "k3s" && length(var.k3s_worker_nodes) > 0 ? 1 : 0

  worker_nodes = [for node in var.k3s_worker_nodes : {
    name                 = node.name
    host                 = node.host
    ssh_user             = coalesce(node.ssh_user, var.ssh_user)
    ssh_private_key_path = coalesce(node.ssh_private_key_path, var.ssh_private_key_path)
    ssh_port             = node.ssh_port
    role                 = node.role
  }]

  server_host                 = var.host_address
  server_ssh_user             = var.ssh_user
  server_ssh_private_key_path = var.ssh_private_key_path
  server_ssh_port             = var.ssh_port
  node_token                  = module.k3s_bootstrap[0].node_token
  k3s_version                 = var.k3s_version
  project_name                = var.project_name
}
