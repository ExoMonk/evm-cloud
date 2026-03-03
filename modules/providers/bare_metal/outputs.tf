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
      bare_metal = var.compute_engine == "docker_compose" ? {
        host_address = var.host_address
        ssh_user     = var.ssh_user
        ssh_port     = var.ssh_port
        ssh_command  = "ssh -p ${var.ssh_port} ${var.ssh_user}@${var.host_address}"
        config_dir   = "/opt/evm-cloud/config"
        compose_file = "/opt/evm-cloud/docker-compose.yml"
      } : null

      k3s = var.compute_engine == "k3s" ? {
        host_address      = var.host_address
        cluster_endpoint  = module.k3s_bootstrap[0].cluster_endpoint
        kubeconfig_base64 = module.k3s_bootstrap[0].kubeconfig_base64
        node_name         = module.k3s_bootstrap[0].node_name
        worker_nodes      = length(var.k3s_worker_nodes) > 0 ? module.k3s_agent[0].worker_nodes : []
      } : null
    }

    secrets = {
      mode              = var.secrets_mode
      eso_chart_version = var.secrets_mode != "inline" ? var.eso_chart_version : null

      provider = null # provider mode not supported on bare_metal

      external = var.secrets_mode == "external" ? {
        store_name = var.external_secret_store_name
        store_kind = "ClusterSecretStore"
        secret_key = var.external_secret_key
      } : null
    }

    services = {
      rpc_proxy = var.rpc_proxy_enabled ? {
        service_name = "erpc"
        port         = 4000
        # k3s: Helm chart creates a Service named <project>-erpc
        # docker_compose: containers communicate via container name
        internal_url = var.compute_engine == "k3s" ? "http://${var.project_name}-erpc:4000" : "http://erpc:4000"
      } : null

      indexer = var.indexer_enabled ? merge({
        service_name           = "rindexer"
        single_writer_required = true
        storage_backend        = var.indexer_storage_backend
        }, length(var.indexer_instances) > 0 ? {
        instances = var.indexer_instances
      } : {}) : null
    }

    data = {
      backend = var.indexer_enabled ? var.indexer_storage_backend : null

      postgres = (var.indexer_enabled && var.indexer_storage_backend == "postgres" && var.indexer_postgres_url != "") ? {
        url = var.indexer_postgres_url
      } : null

      clickhouse = (var.indexer_enabled && var.indexer_storage_backend == "clickhouse") ? {
        url  = var.indexer_clickhouse_url
        user = var.indexer_clickhouse_user
        db   = var.indexer_clickhouse_db
      } : null
    }

    artifacts = {
      config_channel = var.compute_engine == "k3s" ? "helm" : "ssh"
    }
  }
}
