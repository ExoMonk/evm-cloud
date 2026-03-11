terraform {
  required_version = ">= 1.5.0"
}

module "evm_cloud" {
  source = "../.."

  project_name            = var.project_name
  infrastructure_provider = "bare_metal"
  deployment_target       = "self_hosted"
  runtime_arch            = "multi"
  database_mode           = "self_hosted"
  streaming_mode          = "disabled"
  ingress_mode            = "cloudflare"

  # Cloudflare TLS (when ingress_mode = "cloudflare")
  erpc_hostname                  = var.erpc_hostname
  ingress_cloudflare_origin_cert = var.ingress_cloudflare_origin_cert
  ingress_cloudflare_origin_key  = var.ingress_cloudflare_origin_key

  # k3s compute engine — two-phase deployment
  compute_engine = "k3s"
  workload_mode  = "external"

  # Bare metal SSH connection
  bare_metal_host      = var.bare_metal_host
  bare_metal_ssh_user  = var.bare_metal_ssh_user
  ssh_private_key_path = var.ssh_private_key_path
  bare_metal_ssh_port  = var.bare_metal_ssh_port

  # k3s
  k3s_version = var.k3s_version

  # RPC Proxy (eRPC) — Ethereum + Base
  rpc_proxy_enabled = true
  rpc_proxy_image   = var.rpc_proxy_image

  # Indexer — Uniswap V4 Swap events (Ethereum + Base) → ClickHouse
  indexer_enabled         = true
  indexer_image           = var.indexer_image
  indexer_rpc_url         = ""
  indexer_storage_backend = "clickhouse"

  indexer_clickhouse_url      = var.indexer_clickhouse_url
  indexer_clickhouse_user     = var.indexer_clickhouse_user
  indexer_clickhouse_password = var.indexer_clickhouse_password
  indexer_clickhouse_db       = var.indexer_clickhouse_db

  # Config injection
  erpc_config_yaml     = file("${path.module}/config/erpc.yaml")
  rindexer_config_yaml = file("${path.module}/config/rindexer.yaml")
  rindexer_abis = {
    "UniswapV4PoolManager.json" = file("${path.module}/config/abis/UniswapV4PoolManager.json")
  }

  # Indexer env vars — webhook stream to swap-api
  indexer_extra_env = {
    WEBHOOK_URL = "http://${var.project_name}-swap-api:3000"
  }
  indexer_extra_secret_env = {
    RINDEXER_WEBHOOK_SECRET = var.webhook_secret
  }

  # Secrets — inline mode (simplest for bare metal)
  secrets_mode = "inline"

  # Custom service: Swap API (receives webhook stream + queries ClickHouse)
  custom_services = [
    {
      name             = "swap-api"
      image            = var.swap_api_image
      port             = 3000
      health_path      = "/health"
      replicas         = 1
      memory_request   = "128Mi"
      memory_limit     = "256Mi"
      cpu_request      = "100m"
      cpu_limit        = "250m"
      ingress_hostname = var.api_hostname
      env = {
        LOG_LEVEL       = "info"
        WHALE_THRESHOLD = var.whale_threshold
      }
      secret_env = {
        WEBHOOK_SECRET = var.webhook_secret
      }
    }
  ]
}

output "provider_selection" {
  value = module.evm_cloud.provider_selection
}

output "capability_contract" {
  value = module.evm_cloud.capability_contract
}

output "workload_handoff" {
  value     = module.evm_cloud.workload_handoff
  sensitive = true
}
