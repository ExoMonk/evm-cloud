variable "project_name" {
  description = "Project identifier used for naming resources."
  type        = string

  validation {
    condition     = length(trimspace(var.project_name)) > 0
    error_message = "project_name must be a non-empty string."
  }
}

variable "infrastructure_provider" {
  description = "Provider adapter to use. Implemented: aws, bare_metal."
  type        = string
  default     = "aws"
}

variable "deployment_target" {
  description = "High-level deployment mode."
  type        = string
  default     = "managed"

  validation {
    condition     = contains(["managed", "hybrid", "self_hosted"], var.deployment_target)
    error_message = "deployment_target must be one of: managed, hybrid, self_hosted."
  }
}

variable "runtime_arch" {
  description = "Runtime architecture intent for workloads."
  type        = string
  default     = "multi"

  validation {
    condition     = contains(["amd64", "arm64", "multi"], var.runtime_arch)
    error_message = "runtime_arch must be one of: amd64, arm64, multi."
  }
}

variable "database_mode" {
  description = "Database operating mode."
  type        = string
  default     = "managed"

  validation {
    condition     = contains(["managed", "self_hosted"], var.database_mode)
    error_message = "database_mode must be one of: managed, self_hosted."
  }
}

variable "streaming_mode" {
  description = "Streaming operating mode."
  type        = string
  default     = "disabled"

  validation {
    condition     = contains(["managed", "self_hosted", "disabled"], var.streaming_mode)
    error_message = "streaming_mode must be one of: managed, self_hosted, disabled."
  }
}

variable "ingress_mode" {
  description = "Ingress operating mode: none (no TLS), cloudflare (recommended: CF proxy + origin cert), caddy (Let's Encrypt), ingress_nginx (cert-manager)."
  type        = string
  default     = "none"

  validation {
    condition     = contains(["none", "cloudflare", "caddy", "ingress_nginx"], var.ingress_mode)
    error_message = "ingress_mode must be one of: none, cloudflare, caddy, ingress_nginx."
  }
}

variable "erpc_hostname" {
  description = "Public hostname for eRPC TLS certificate and routing (e.g. rpc.example.com). Required when ingress_mode != none."
  type        = string
  default     = ""

  validation {
    condition     = var.erpc_hostname == "" || can(regex("^[a-z0-9][a-z0-9.-]*[a-z0-9]$", var.erpc_hostname))
    error_message = "erpc_hostname must be a valid hostname (e.g., rpc.example.com), not a URL or IP address."
  }
}

variable "ingress_tls_email" {
  description = "Email for Let's Encrypt certificate registration. Required when ingress_mode = caddy or ingress_nginx."
  type        = string
  default     = ""
}

variable "ingress_cloudflare_origin_cert" {
  description = "Cloudflare Origin Certificate (PEM). Required when ingress_mode = cloudflare. Generate at Cloudflare dashboard > SSL/TLS > Origin Server."
  type        = string
  default     = ""
  sensitive   = true
}

variable "ingress_cloudflare_origin_key" {
  description = "Cloudflare Origin Certificate private key (PEM). Required when ingress_mode = cloudflare."
  type        = string
  default     = ""
  sensitive   = true
}

variable "ingress_cloudflare_ssl_mode" {
  description = "Cloudflare SSL/TLS encryption mode. full_strict requires valid origin cert (recommended). full accepts self-signed."
  type        = string
  default     = "full_strict"

  validation {
    condition     = contains(["full", "full_strict"], var.ingress_cloudflare_ssl_mode)
    error_message = "ingress_cloudflare_ssl_mode must be full or full_strict."
  }
}

variable "ingress_caddy_image" {
  description = "Container image for Caddy reverse proxy."
  type        = string
  default     = "caddy:2.9.1-alpine"
}

variable "ingress_caddy_mem_limit" {
  description = "Docker memory limit for Caddy container (e.g. 128m, 256m)."
  type        = string
  default     = "128m"
}

variable "ingress_nginx_chart_version" {
  description = "ingress-nginx Helm chart version. Pinned for reproducibility."
  type        = string
  default     = "4.11.3"
}

variable "ingress_cert_manager_chart_version" {
  description = "cert-manager Helm chart version. Pinned for reproducibility."
  type        = string
  default     = "1.16.2"
}

variable "ingress_request_body_max_size" {
  description = "Maximum request body size for ingress (e.g. 1m, 10m). Applied as Caddy max_size or nginx annotation."
  type        = string
  default     = "1m"
}

variable "ingress_tls_staging" {
  description = "Use Let's Encrypt staging ACME server. Recommended for testing to avoid rate limits."
  type        = bool
  default     = false
}

variable "ingress_hsts_preload" {
  description = "Add 'preload' to HSTS header. WARNING: Once submitted to hstspreload.org, this is extremely difficult to reverse. Only enable for production domains."
  type        = bool
  default     = false
}

variable "workload_mode" {
  description = "Workload ownership: terraform (default) manages app resources, external delegates to CI/GitOps tools."
  type        = string
  default     = "terraform"

  validation {
    condition     = contains(["terraform", "external"], var.workload_mode)
    error_message = "workload_mode must be one of: terraform, external."
  }
}

variable "compute_engine" {
  description = "Compute engine for workloads: ec2/eks/k3s (AWS), docker_compose/k3s (bare_metal). Changing on an existing deployment destroys and recreates all compute resources; database is preserved."
  type        = string
  default     = "ec2"

  validation {
    condition     = contains(["ec2", "eks", "docker_compose", "k3s"], var.compute_engine)
    error_message = "compute_engine must be one of: ec2, eks, docker_compose, k3s."
  }
}

variable "ssh_public_key" {
  description = "SSH public key for EC2 deploy key pair. Required when compute_engine=ec2."
  type        = string
  default     = ""
  sensitive   = true
}

variable "ssh_private_key_path" {
  description = "Path to SSH private key file used for provisioning and config updates. Required for EC2, K3s, and bare metal deployments."
  type        = string
  default     = ""
  sensitive   = true
}

variable "ec2_instance_type" {
  description = "EC2 instance type for Docker Compose compute engine."
  type        = string
  default     = "t3.small"
}

variable "ec2_rpc_proxy_mem_limit" {
  description = "Docker memory limit for eRPC container on EC2 (e.g. 512m, 1g, 2g)."
  type        = string
  default     = "1g"
}

variable "ec2_indexer_mem_limit" {
  description = "Docker memory limit for rindexer container on EC2 (e.g. 1g, 2g, 4g)."
  type        = string
  default     = "2g"
}

variable "ec2_secret_recovery_window_in_days" {
  description = "Recovery window for Secrets Manager secret deletion (0 = immediate for dev, 7-30 for production)."
  type        = number
  default     = 7
}

variable "networking_enabled" {
  description = "Enable AWS networking module provisioning in the provider adapter."
  type        = bool
  default     = false
}

variable "network_environment" {
  description = "Networking profile environment."
  type        = string
  default     = "dev"

  validation {
    condition     = contains(["dev", "production", "platform"], var.network_environment)
    error_message = "network_environment must be one of: dev, production, platform."
  }
}

variable "aws_region" {
  description = "AWS region for provider-backed resources."
  type        = string
  default     = "us-east-1"
}

variable "aws_skip_credentials_validation" {
  description = "Skip AWS provider credential/account validation checks (useful for local simulation)."
  type        = bool
  default     = false
}

variable "network_vpc_cidr" {
  description = "VPC CIDR block for networking module."
  type        = string
  default     = "10.42.0.0/16"
}

variable "network_availability_zones" {
  description = "Availability zones used for subnets."
  type        = list(string)
  default     = ["us-east-1a", "us-east-1b"]
}

variable "network_enable_nat_gateway" {
  description = "Enable NAT gateway for private subnet egress."
  type        = bool
  default     = false
}

variable "network_enable_vpc_endpoints" {
  description = "Enable baseline VPC endpoints (S3 + interface endpoints)."
  type        = bool
  default     = false
}

# --- Postgres ---

variable "postgres_enabled" {
  description = "Enable managed PostgreSQL provisioning."
  type        = bool
  default     = false
}

variable "postgres_instance_class" {
  description = "RDS instance class for PostgreSQL."
  type        = string
  default     = "db.t4g.micro"
}

variable "postgres_engine_version" {
  description = "PostgreSQL engine version."
  type        = string
  default     = "16.4"
}

variable "postgres_db_name" {
  description = "Database name to create."
  type        = string
  default     = "rindexer"
}

variable "postgres_db_username" {
  description = "Master username for PostgreSQL."
  type        = string
  default     = "rindexer"
}

variable "postgres_backup_retention" {
  description = "Backup retention period in days."
  type        = number
  default     = 7
}

variable "postgres_manage_master_user_password" {
  description = "Let AWS manage the RDS master password via Secrets Manager (automatic rotation, 7-day recovery window). Set to false and provide postgres_master_password for explicit control."
  type        = bool
  default     = true
}

variable "postgres_master_password" {
  description = "Explicit master password for RDS. Required when postgres_manage_master_user_password = false."
  type        = string
  default     = null
  sensitive   = true
}

variable "postgres_force_ssl" {
  description = "Require SSL for all RDS connections. Set to false for clients with broken TLS (e.g. rindexer < 0.37 using native-tls)."
  type        = bool
  default     = false
}

# --- RPC Proxy (eRPC) ---

variable "rpc_proxy_enabled" {
  description = "Enable eRPC proxy deployment."
  type        = bool
  default     = false
}

variable "rpc_proxy_image" {
  description = "Container image for eRPC. Override for multi-arch compatibility."
  type        = string
  default     = "ghcr.io/erpc/erpc:latest"
}

# --- Indexer (rindexer) ---

variable "indexer_enabled" {
  description = "Enable rindexer indexer deployment."
  type        = bool
  default     = false
}

variable "indexer_image" {
  description = "Container image for rindexer. Override for multi-arch compatibility."
  type        = string
  default     = "ghcr.io/joshstevens19/rindexer:latest"
}

variable "indexer_rpc_url" {
  description = "RPC endpoint URL for the indexer. Injected as RPC_URL env var — reference as $${RPC_URL} in rindexer.yaml networks section."
  type        = string
  default     = ""
}

variable "indexer_storage_backend" {
  description = "Storage backend for rindexer: postgres (managed RDS) or clickhouse (BYODB). Cannot use both — rindexer limitation."
  type        = string
  default     = "postgres"

  validation {
    condition     = contains(["postgres", "clickhouse"], var.indexer_storage_backend)
    error_message = "indexer_storage_backend must be one of: postgres, clickhouse."
  }
}

# --- ClickHouse BYODB ---

variable "indexer_postgres_url" {
  description = "PostgreSQL connection string (e.g. postgres://user:pass@host:5432/db). Used for bare_metal + postgres deployments."
  type        = string
  default     = ""
  sensitive   = true
}

variable "indexer_clickhouse_url" {
  description = "ClickHouse HTTP endpoint (e.g. http://clickhouse.example.com:8123). Required when indexer_storage_backend=clickhouse."
  type        = string
  default     = ""
  sensitive   = true
}

variable "indexer_clickhouse_user" {
  description = "ClickHouse username."
  type        = string
  default     = "default"
}

variable "indexer_clickhouse_password" {
  description = "ClickHouse password."
  type        = string
  default     = ""
  sensitive   = true
}

variable "indexer_clickhouse_db" {
  description = "ClickHouse database name."
  type        = string
  default     = "default"
}

# --- Config injection ---

variable "erpc_config_yaml" {
  description = "Full erpc.yaml content. Required when rpc_proxy_enabled=true. eRPC reads this file, not env vars."
  type        = string
  default     = ""
}

variable "rindexer_config_yaml" {
  description = "Full rindexer.yaml content. Required when indexer_enabled=true. Use $${RPC_URL} and $${DATABASE_URL} for runtime interpolation."
  type        = string
  default     = ""
}

variable "rindexer_abis" {
  description = "Map of ABI filename to JSON content, e.g. { \"ERC20.json\" = file(\"abis/ERC20.json\") }. Deployed alongside rindexer.yaml."
  type        = map(string)
  default     = {}
}

variable "indexer_instances" {
  description = "Multiple indexer instances with independent configs. Empty = single instance (backward compat). Each instance becomes a separate Helm release."
  type = list(object({
    name          = string
    config_key    = optional(string)
    node_role     = optional(string)
    workload_type = optional(string) # "deployment" (default) or "job" (one-shot backfill)
  }))
  default = []
}

# --- Bare Metal ---

variable "bare_metal_host" {
  description = "IP or hostname of the VPS. Required when infrastructure_provider=bare_metal."
  type        = string
  default     = ""
}

variable "bare_metal_ssh_user" {
  description = "SSH user for bare metal host."
  type        = string
  default     = "ubuntu"
}

variable "bare_metal_ssh_port" {
  description = "SSH port for bare metal host."
  type        = number
  default     = 22
}

variable "bare_metal_rpc_proxy_mem_limit" {
  description = "Docker memory limit for eRPC container on bare metal (e.g. 512m, 1g, 2g)."
  type        = string
  default     = "1g"
}

variable "bare_metal_indexer_mem_limit" {
  description = "Docker memory limit for rindexer container on bare metal (e.g. 1g, 2g, 4g)."
  type        = string
  default     = "2g"
}

# --- k3s ---

variable "k3s_version" {
  description = "k3s version to install (e.g., v1.30.4+k3s1). Pinned for reproducibility."
  type        = string
  default     = "v1.30.4+k3s1"
}

variable "k3s_instance_type" {
  description = "EC2 instance type for k3s host when infrastructure_provider=aws."
  type        = string
  default     = "t3.small"
}

variable "k3s_api_allowed_cidrs" {
  description = "CIDR blocks allowed to access k3s API (port 6443). Defaults to VPC CIDR when empty and networking is enabled."
  type        = list(string)
  default     = []
}

variable "k3s_worker_nodes" {
  description = "Worker nodes to join the k3s cluster. For AWS: each gets a dedicated EC2 instance (use instance_type, use_spot). For bare_metal: each must have a host address."
  type = list(object({
    name          = string
    role          = optional(string, "general")
    instance_type = optional(string, "t3.small")
    use_spot      = optional(bool, false)
    host          = optional(string)
  }))
  default = []
}

# --- Secrets Management ---

variable "secrets_mode" {
  description = "How secrets are delivered to workloads: inline (default, current behavior), provider (AWS Secrets Manager + ESO), or external (user-managed secret store)."
  type        = string
  default     = "inline"

  validation {
    condition     = contains(["inline", "provider", "external"], var.secrets_mode)
    error_message = "secrets_mode must be one of: inline, provider, external."
  }
}

variable "secrets_manager_secret_arn" {
  description = "ARN of a pre-existing AWS Secrets Manager secret. When set, Terraform skips creating the secret (avoids secret values in TF state). Required format: arn:aws:secretsmanager:REGION:ACCOUNT:secret:NAME."
  type        = string
  default     = ""
  sensitive   = true
}

variable "secrets_manager_kms_key_id" {
  description = "KMS key ID or ARN for encrypting the Secrets Manager secret. Empty uses the AWS-managed key."
  type        = string
  default     = ""
}

variable "external_secret_store_name" {
  description = "Name of a user-managed ClusterSecretStore for secrets_mode=external. The store must exist in the cluster before deploying workloads."
  type        = string
  default     = ""
}

variable "external_secret_key" {
  description = "Secret key/name in the external store that holds workload env vars. Required when secrets_mode=external."
  type        = string
  default     = ""
}

variable "eso_chart_version" {
  description = "External Secrets Operator Helm chart version. Pinned for reproducibility."
  type        = string
  default     = "0.9.13"
}

# --- Monitoring ---

variable "monitoring_enabled" {
  description = "Enable kube-prometheus-stack monitoring addon. Requires compute_engine = eks or k3s."
  type        = bool
  default     = false
}

variable "kube_prometheus_stack_version" {
  description = "kube-prometheus-stack Helm chart version."
  type        = string
  default     = "72.6.2"
}

variable "grafana_admin_password_secret_name" {
  description = "Existing K8s Secret name for Grafana admin password (keys: admin-user, admin-password). Empty = chart default."
  type        = string
  default     = ""
}

variable "alertmanager_slack_webhook_secret_name" {
  description = "Existing K8s Secret name holding Slack webhook URL (key: webhook_url)."
  type        = string
  default     = ""
}

variable "alertmanager_sns_topic_arn" {
  description = "SNS topic ARN for Alertmanager webhook routing."
  type        = string
  default     = ""
}

variable "alertmanager_pagerduty_routing_key_secret_name" {
  description = "Existing K8s Secret name holding PagerDuty routing key (key: routing_key)."
  type        = string
  default     = ""
}

variable "alertmanager_route_target" {
  description = "Alertmanager receiver target: slack, sns, or pagerduty."
  type        = string
  default     = "slack"

  validation {
    condition     = contains(["slack", "sns", "pagerduty"], var.alertmanager_route_target)
    error_message = "alertmanager_route_target must be one of: slack, sns, pagerduty."
  }
}

variable "alertmanager_slack_channel" {
  description = "Slack channel name for alert delivery."
  type        = string
  default     = "#alerts"
}

variable "loki_enabled" {
  description = "Deploy Loki + Promtail for log aggregation. Requires monitoring_enabled = true."
  type        = bool
  default     = false
}

variable "loki_chart_version" {
  description = "Loki Helm chart version."
  type        = string
  default     = "6.24.0"
}

variable "promtail_chart_version" {
  description = "Promtail Helm chart version."
  type        = string
  default     = "6.16.6"
}

variable "loki_persistence_enabled" {
  description = "Enable PVC for Loki. False = logs lost on pod restart."
  type        = bool
  default     = false
}

variable "clickhouse_metrics_url" {
  description = "Optional BYO ClickHouse metrics endpoint for Prometheus scraping."
  type        = string
  default     = ""
}

variable "grafana_ingress_enabled" {
  description = "Expose Grafana via Ingress. Requires ingress_mode != none and grafana_hostname set."
  type        = bool
  default     = true
}

variable "grafana_hostname" {
  description = "Grafana hostname for Ingress (e.g., grafana.yourdomain.com)."
  type        = string
  default     = ""
}

variable "ingress_class_name" {
  description = "Ingress class name for Grafana Ingress resource."
  type        = string
  default     = "nginx"
}

variable "bare_metal_secrets_encryption" {
  description = "Encryption method for bare metal .env secrets: none (default) or sops_age (SOPS + age encryption at rest)."
  type        = string
  default     = "none"

  validation {
    condition     = contains(["none", "sops_age"], var.bare_metal_secrets_encryption)
    error_message = "bare_metal_secrets_encryption must be one of: none, sops_age."
  }
}

