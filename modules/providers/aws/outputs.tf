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
      } : null
    }

    services = {
      rpc_proxy = var.rpc_proxy_enabled ? {
        service_name = var.compute_engine == "ec2" ? "erpc" : "${var.project_name}-erpc"
        port         = 4000
        internal_url = var.compute_engine == "ec2" ? "http://erpc:4000" : null
      } : null

      indexer = var.indexer_enabled ? {
        service_name           = var.compute_engine == "ec2" ? "rindexer" : "${var.project_name}-indexer"
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
        url      = var.indexer_clickhouse_url
        user     = var.indexer_clickhouse_user
        db       = var.indexer_clickhouse_db
        password = var.compute_engine == "k3s" ? var.indexer_clickhouse_password : null
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
