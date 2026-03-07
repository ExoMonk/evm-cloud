output "adapter_context" {
  description = "Resolved AWS adapter context from provider-neutral root inputs."
  value       = local.adapter_context
}

output "networking" {
  description = "Networking outputs from AWS adapter, or null when disabled."
  value       = local.networking
}

output "postgres" {
  description = "PostgreSQL outputs; null when disabled."
  value = var.postgres_enabled ? {
    endpoint          = module.postgres[0].endpoint
    port              = module.postgres[0].port
    db_name           = module.postgres[0].db_name
    master_secret_arn = module.postgres[0].master_secret_arn
  } : null
}

output "eks_cluster_name" {
  description = "EKS cluster name; empty when compute_engine != eks."
  value       = local.any_eks_compute ? module.eks_cluster[0].cluster_name : ""
}

output "rpc_proxy" {
  description = "eRPC proxy outputs; null when disabled or external+EKS."
  value = (var.rpc_proxy_enabled && (var.compute_engine == "ec2" || local.terraform_manages_workloads)) ? {
    service_name = (
      var.compute_engine == "ec2" ? "erpc"
      : module.eks_rpc_proxy[0].service_name
    )
    container_port = (
      var.compute_engine == "ec2" ? 4000
      : module.eks_rpc_proxy[0].container_port
    )
  } : null
}

output "indexer" {
  description = "rindexer indexer outputs; null when disabled or external+EKS."
  value = (var.indexer_enabled && (var.compute_engine == "ec2" || local.terraform_manages_workloads)) ? {
    service_name = (
      var.compute_engine == "ec2" ? "rindexer"
      : module.eks_indexer[0].service_name
    )
    log_group_name = (
      var.compute_engine == "ec2" ? module.ec2[0].cloudwatch_log_group
      : module.eks_indexer[0].log_group_name
    )
  } : null
}

output "workload_handoff" {
  description = "Handoff contract for external deployers. Contains all info needed to deploy workloads outside Terraform."
  value = {
    version        = "v1"
    mode           = var.workload_mode
    compute_engine = var.compute_engine
    project_name   = var.project_name
    aws_region     = var.aws_region

    identity = {
      ec2_instance_profile = (var.compute_engine == "ec2" && local.any_ec2_compute) ? {
        name     = aws_iam_instance_profile.ec2[0].name
        role_arn = aws_iam_role.ec2_instance[0].arn
      } : null

      eks_irsa_role_arns = var.compute_engine == "eks" ? {
        rpc_proxy = null
        indexer   = null
      } : null
    }

    network = local.networking != null ? {
      vpc_id             = local.networking.vpc_id
      public_subnet_ids  = local.networking.public_subnet_ids
      private_subnet_ids = local.networking.private_subnet_ids
      security_groups = {
        rpc_proxy = var.rpc_proxy_enabled ? local.networking.security_group_ids["erpc"] : null
        indexer   = var.indexer_enabled ? local.networking.security_group_ids["indexer"] : null
      }
    } : null

    runtime = {
      ec2 = var.compute_engine == "ec2" ? {
        instance_id          = local.any_ec2_compute ? module.ec2[0].instance_id : null
        public_ip            = local.any_ec2_compute ? module.ec2[0].instance_public_ip : null
        ssh_command          = local.any_ec2_compute ? module.ec2[0].ssh_command : null
        config_dir           = "/opt/evm-cloud/config"
        compose_file         = "/opt/evm-cloud/docker-compose.yml"
        secret_arn           = local.any_ec2_compute ? module.ec2[0].secret_arn : null
        cloudwatch_log_group = local.any_ec2_compute ? module.ec2[0].cloudwatch_log_group : null
      } : null

      eks = var.compute_engine == "eks" ? {
        cluster_name      = local.any_eks_compute ? module.eks_cluster[0].cluster_name : null
        cluster_endpoint  = local.any_eks_compute ? module.eks_cluster[0].cluster_endpoint : null
        oidc_provider_arn = local.any_eks_compute ? module.eks_cluster[0].oidc_provider_arn : null
      } : null

      k3s = var.compute_engine == "k3s" ? {
        host_ip           = local.any_k3s_compute ? module.k3s_host[0].host_ip : null
        instance_id       = local.any_k3s_compute ? module.k3s_host[0].instance_id : null
        cluster_endpoint  = local.any_k3s_compute ? module.k3s_bootstrap[0].cluster_endpoint : null
        kubeconfig_base64 = local.any_k3s_compute ? module.k3s_bootstrap[0].kubeconfig_base64 : null
        node_name         = local.any_k3s_compute ? module.k3s_bootstrap[0].node_name : null
        worker_nodes      = (local.any_k3s_compute && length(var.k3s_worker_nodes) > 0) ? module.k3s_agent[0].worker_nodes : []
      } : null
    }

    ingress = {
      mode                       = var.ingress_mode
      erpc_hostname              = var.ingress_mode != "none" ? var.erpc_hostname : null
      tls_email                  = contains(["caddy", "ingress_nginx"], var.ingress_mode) ? var.ingress_tls_email : null
      tls_staging                = var.ingress_tls_staging
      hsts_preload               = var.ingress_hsts_preload
      request_body_max_size      = var.ingress_request_body_max_size
      caddy_image                = var.ingress_mode == "caddy" ? var.ingress_caddy_image : null
      caddy_mem_limit            = var.ingress_mode == "caddy" ? var.ingress_caddy_mem_limit : null
      nginx_chart_version        = var.ingress_mode == "ingress_nginx" ? var.ingress_nginx_chart_version : null
      cert_manager_chart_version = var.ingress_mode == "ingress_nginx" ? var.ingress_cert_manager_chart_version : null

      cloudflare = var.ingress_mode == "cloudflare" ? {
        origin_cert = var.ingress_cloudflare_origin_cert
        origin_key  = var.ingress_cloudflare_origin_key
        ssl_mode    = var.ingress_cloudflare_ssl_mode
      } : null
    }

    secrets = {
      mode              = var.secrets_mode
      eso_chart_version = var.secrets_mode != "inline" ? var.eso_chart_version : null

      provider = var.secrets_mode == "provider" ? {
        type       = "aws_sm"
        secret_arn = local.workload_secret_arn
        region     = var.aws_region
      } : null

      external = var.secrets_mode == "external" ? {
        store_name = var.external_secret_store_name
        store_kind = "ClusterSecretStore"
        secret_key = var.external_secret_key
      } : null
    }

    services = {
      rpc_proxy = var.rpc_proxy_enabled ? {
        service_name = var.compute_engine == "ec2" ? "erpc" : "${var.project_name}-erpc"
        port         = 4000
        internal_url = var.compute_engine == "ec2" ? "http://erpc:4000" : null
      } : null

      indexer = var.indexer_enabled ? merge({
        service_name           = var.compute_engine == "ec2" ? "rindexer" : "${var.project_name}-indexer"
        single_writer_required = true
        storage_backend        = var.indexer_storage_backend
        extra_env              = var.indexer_extra_env
        }, length(var.indexer_instances) > 0 ? {
        instances = var.indexer_instances
      } : {}) : null

      monitoring = var.monitoring_enabled ? {
        kube_prometheus_stack_version                  = var.kube_prometheus_stack_version
        grafana_admin_password_secret_name             = var.grafana_admin_password_secret_name
        alertmanager_route_target                      = var.alertmanager_route_target
        alertmanager_slack_webhook_secret_name         = var.alertmanager_slack_webhook_secret_name
        alertmanager_slack_channel                     = var.alertmanager_slack_channel
        alertmanager_sns_topic_arn                     = var.alertmanager_sns_topic_arn
        alertmanager_pagerduty_routing_key_secret_name = var.alertmanager_pagerduty_routing_key_secret_name
        loki_enabled                                   = var.loki_enabled
        loki_chart_version                             = var.loki_chart_version
        promtail_chart_version                         = var.promtail_chart_version
        loki_persistence_enabled                       = var.loki_persistence_enabled
        clickhouse_metrics_url                         = var.clickhouse_metrics_url
        grafana_ingress_enabled                        = var.grafana_ingress_enabled
        grafana_hostname                               = var.grafana_hostname
        ingress_class_name                             = var.ingress_class_name
      } : null
    }

    data = {
      backend = var.indexer_enabled ? var.indexer_storage_backend : null

      postgres = (var.indexer_enabled && var.indexer_storage_backend == "postgres" && var.postgres_enabled) ? {
        host       = module.postgres[0].endpoint
        port       = module.postgres[0].port
        db_name    = module.postgres[0].db_name
        secret_arn = module.postgres[0].master_secret_arn
        url        = local.managed_database_url
      } : null

      clickhouse = (var.indexer_enabled && var.indexer_storage_backend == "clickhouse") ? {
        url  = var.indexer_clickhouse_url
        user = var.indexer_clickhouse_user
        db   = var.indexer_clickhouse_db
        # Password only included in handoff when secrets_mode=inline (backward compat)
        password = (var.compute_engine == "k3s" && var.secrets_mode == "inline") ? var.indexer_clickhouse_password : null
      } : null
    }

    artifacts = {
      config_channel = (
        var.compute_engine == "ec2" ? "ssh"
        : var.compute_engine == "k3s" ? "helm"
        : "k8s_config"
      )
    }
  }
}
