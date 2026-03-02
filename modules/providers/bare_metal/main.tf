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
    var.indexer_storage_backend == "postgres" ? {} : {},
    var.indexer_storage_backend == "clickhouse" ? {
      CLICKHOUSE_URL      = var.indexer_clickhouse_url
      CLICKHOUSE_USER     = var.indexer_clickhouse_user
      CLICKHOUSE_PASSWORD = var.indexer_clickhouse_password
      CLICKHOUSE_DB       = var.indexer_clickhouse_db
    } : {},
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
