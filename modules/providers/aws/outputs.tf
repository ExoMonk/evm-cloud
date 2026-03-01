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
  description = "eRPC proxy outputs; null when disabled or workload_mode=external."
  value = (var.rpc_proxy_enabled && local.terraform_manages_workloads) ? {
    service_name   = var.compute_engine == "ecs" ? module.rpc_proxy[0].service_name : module.eks_rpc_proxy[0].service_name
    container_port = var.compute_engine == "ecs" ? module.rpc_proxy[0].container_port : module.eks_rpc_proxy[0].container_port
  } : null
}

output "indexer" {
  description = "rindexer indexer outputs; null when disabled or workload_mode=external."
  value = (var.indexer_enabled && local.terraform_manages_workloads) ? {
    service_name   = var.compute_engine == "ecs" ? module.indexer[0].service_name : module.eks_indexer[0].service_name
    log_group_name = var.compute_engine == "ecs" ? module.indexer[0].log_group_name : module.eks_indexer[0].log_group_name
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
      ecs_task_execution_role_arn = (var.compute_engine == "ecs" && local.any_ecs_compute) ? aws_iam_role.ecs_task_execution[0].arn : null

      ecs_task_role_arns = (var.compute_engine == "ecs" && local.any_ecs_compute) ? {
        rpc_proxy = var.rpc_proxy_enabled ? aws_iam_role.ecs_task_rpc_proxy[0].arn : null
        indexer   = var.indexer_enabled ? aws_iam_role.ecs_task_indexer[0].arn : null
      } : null

      eks_irsa_role_arns = var.compute_engine == "eks" ? {
        rpc_proxy = null
        indexer   = null
      } : null
    }

    network = local.networking != null ? {
      vpc_id             = local.networking.vpc_id
      private_subnet_ids = local.networking.private_subnet_ids
      security_groups = {
        rpc_proxy = var.rpc_proxy_enabled ? local.networking.security_group_ids["erpc"] : null
        indexer   = var.indexer_enabled ? local.networking.security_group_ids["indexer"] : null
      }
    } : null

    runtime = {
      ecs = var.compute_engine == "ecs" ? {
        cluster_arn = local.any_ecs_compute ? module.ecs_cluster[0].arn : null
        service_discovery = local.any_ecs_compute ? {
          namespace_id   = aws_service_discovery_private_dns_namespace.ecs[0].id
          namespace_name = aws_service_discovery_private_dns_namespace.ecs[0].name
        } : null
      } : null

      eks = var.compute_engine == "eks" ? {
        cluster_name      = local.any_eks_compute ? module.eks_cluster[0].cluster_name : null
        cluster_endpoint  = local.any_eks_compute ? module.eks_cluster[0].cluster_endpoint : null
        oidc_provider_arn = local.any_eks_compute ? module.eks_cluster[0].oidc_provider_arn : null
      } : null
    }

    services = {
      rpc_proxy = var.rpc_proxy_enabled ? {
        service_name = "${var.project_name}-erpc"
        port         = 4000
        discovery = (var.compute_engine == "ecs" && local.any_ecs_compute) ? {
          service_arn    = aws_service_discovery_service.rpc_proxy[0].arn
          service_name   = aws_service_discovery_service.rpc_proxy[0].name
          namespace_name = aws_service_discovery_private_dns_namespace.ecs[0].name
          internal_url   = "http://${aws_service_discovery_service.rpc_proxy[0].name}.${aws_service_discovery_private_dns_namespace.ecs[0].name}:4000"
        } : null
      } : null

      indexer = var.indexer_enabled ? {
        service_name           = "${var.project_name}-indexer"
        single_writer_required = true
        storage_backend        = var.indexer_storage_backend
      } : null
    }

    data = {
      backend = var.indexer_enabled ? var.indexer_storage_backend : null

      postgres = (var.indexer_enabled && var.indexer_storage_backend == "postgres" && var.postgres_enabled) ? {
        host       = module.postgres[0].endpoint
        port       = module.postgres[0].port
        db_name    = module.postgres[0].db_name
        secret_arn = module.postgres[0].master_secret_arn
      } : null

      clickhouse = (var.indexer_enabled && var.indexer_storage_backend == "clickhouse") ? {
        url  = var.indexer_clickhouse_url
        user = var.indexer_clickhouse_user
        db   = var.indexer_clickhouse_db
      } : null
    }

    artifacts = {
      config_channel = var.compute_engine == "ecs" ? "s3" : "k8s_config"

      s3 = (var.compute_engine == "ecs" && local.any_ecs_compute) ? {
        bucket               = aws_s3_bucket.config[0].id
        erpc_config_key      = var.rpc_proxy_enabled ? "erpc/erpc.yaml" : null
        rindexer_config_key  = var.indexer_enabled ? "rindexer/rindexer.yaml" : null
        rindexer_abis_prefix = var.indexer_enabled ? "rindexer/abis/" : null
      } : null
    }
  }
}
