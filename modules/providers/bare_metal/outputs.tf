output "adapter_context" {
  description = "Bare metal adapter context."
  value = {
    provider     = "bare_metal"
    project_name = var.project_name
    host_address = var.host_address
    ssh_user     = var.ssh_user
    ssh_port     = var.ssh_port
  }
}

output "networking" {
  description = "Networking outputs — null for bare metal (user-managed)."
  value       = null
}

output "postgres" {
  description = "PostgreSQL outputs — null for bare metal (BYO database)."
  value       = null
}

output "rpc_proxy" {
  description = "eRPC proxy outputs."
  value = var.rpc_proxy_enabled ? {
    service_name   = "erpc"
    container_port = 4000
  } : null
}

output "indexer" {
  description = "rindexer indexer outputs."
  value = var.indexer_enabled ? {
    service_name   = "rindexer"
    log_group_name = ""
  } : null
}

output "workload_handoff" {
  description = "Handoff contract for external deployers."
  value = {
    version        = "v1"
    mode           = var.workload_mode
    compute_engine = var.compute_engine
    project_name   = var.project_name

    identity = null
    network  = null

    runtime = {
      ec2 = null
      eks = null
      bare_metal = {
        host_address = var.host_address
        ssh_user     = var.ssh_user
        ssh_port     = var.ssh_port
        ssh_command  = "ssh -p ${var.ssh_port} ${var.ssh_user}@${var.host_address}"
        config_dir   = "/opt/evm-cloud/config"
        compose_file = "/opt/evm-cloud/docker-compose.yml"
      }
    }

    services = {
      rpc_proxy = var.rpc_proxy_enabled ? {
        service_name = "erpc"
        port         = 4000
        internal_url = "http://erpc:4000"
      } : null

      indexer = var.indexer_enabled ? {
        service_name           = "rindexer"
        single_writer_required = true
        storage_backend        = var.indexer_storage_backend
      } : null
    }

    data = {
      backend = var.indexer_enabled ? var.indexer_storage_backend : null

      postgres = null

      clickhouse = (var.indexer_enabled && var.indexer_storage_backend == "clickhouse") ? {
        url  = var.indexer_clickhouse_url
        user = var.indexer_clickhouse_user
        db   = var.indexer_clickhouse_db
      } : null
    }

    artifacts = {
      config_channel = "ssh"
    }
  }
}
