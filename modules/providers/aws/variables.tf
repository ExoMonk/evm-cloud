variable "project_name" {
  description = "Project identifier used for naming resources."
  type        = string
}

variable "deployment_target" {
  description = "Deployment posture selected at root."
  type        = string
}

variable "runtime_arch" {
  description = "Runtime architecture intent selected at root."
  type        = string
}

variable "database_mode" {
  description = "Database mode selected at root."
  type        = string
}

variable "streaming_mode" {
  description = "Streaming mode selected at root."
  type        = string
}

variable "ingress_mode" {
  description = "Ingress mode selected at root."
  type        = string
}

variable "erpc_hostname" {
  description = "Public hostname for eRPC TLS certificate and routing."
  type        = string
  default     = ""
}

variable "ingress_tls_email" {
  description = "Email for Let's Encrypt certificate registration."
  type        = string
  default     = ""
}

variable "ingress_cloudflare_origin_cert" {
  description = "Cloudflare Origin Certificate (PEM)."
  type        = string
  default     = ""
  sensitive   = true
}

variable "ingress_cloudflare_origin_key" {
  description = "Cloudflare Origin Certificate private key (PEM)."
  type        = string
  default     = ""
  sensitive   = true
}

variable "ingress_cloudflare_ssl_mode" {
  description = "Cloudflare SSL/TLS encryption mode."
  type        = string
  default     = "full_strict"
}

variable "ingress_caddy_image" {
  description = "Container image for Caddy reverse proxy."
  type        = string
  default     = "caddy:2.9.1-alpine"
}

variable "ingress_caddy_mem_limit" {
  description = "Docker memory limit for Caddy container."
  type        = string
  default     = "128m"
}

variable "ingress_nginx_chart_version" {
  description = "ingress-nginx Helm chart version."
  type        = string
  default     = "4.11.3"
}

variable "ingress_cert_manager_chart_version" {
  description = "cert-manager Helm chart version."
  type        = string
  default     = "1.16.2"
}

variable "ingress_request_body_max_size" {
  description = "Maximum request body size for ingress."
  type        = string
  default     = "1m"
}

variable "ingress_tls_staging" {
  description = "Use Let's Encrypt staging ACME server."
  type        = bool
  default     = false
}

variable "ingress_hsts_preload" {
  description = "Add 'preload' to HSTS header."
  type        = bool
  default     = false
}

variable "aws_region" {
  description = "AWS region used by adapter resources."
  type        = string
}

variable "compute_engine" {
  description = "Compute engine for workloads: ec2 (Docker Compose), eks (managed K8s), or k3s (lightweight K8s)."
  type        = string
  default     = "ec2"

  validation {
    condition     = contains(["ec2", "eks", "k3s"], var.compute_engine)
    error_message = "compute_engine must be one of: ec2, eks, k3s."
  }
}

variable "ssh_public_key" {
  description = "SSH public key for EC2 deploy key pair. Required when compute_engine=ec2."
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

variable "ssh_private_key_path" {
  description = "Path to SSH private key file used for provisioning and config updates."
  type        = string
  default     = ""
  sensitive   = true
}

variable "ec2_secret_recovery_window_in_days" {
  description = "Recovery window for Secrets Manager secret deletion (0 = immediate for dev, 7-30 for production)."
  type        = number
  default     = 7
}

variable "workload_mode" {
  description = "Workload ownership: terraform manages app resources, external delegates to CI/GitOps tools."
  type        = string
  default     = "terraform"

  validation {
    condition     = contains(["terraform", "external"], var.workload_mode)
    error_message = "workload_mode must be one of: terraform, external."
  }
}

variable "networking_enabled" {
  description = "Enable networking module provisioning."
  type        = bool
}

variable "network_environment" {
  description = "Networking profile environment."
  type        = string
}

variable "network_vpc_cidr" {
  description = "VPC CIDR for networking module."
  type        = string
}

variable "network_availability_zones" {
  description = "Availability zones for networking module."
  type        = list(string)
}

variable "network_enable_nat_gateway" {
  description = "Enable NAT gateway in networking module."
  type        = bool
}

variable "network_enable_vpc_endpoints" {
  description = "Enable VPC endpoints in networking module."
  type        = bool
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
  description = "Let AWS manage the RDS master password via Secrets Manager. Set to false and provide postgres_master_password for explicit control."
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
  description = "Container image for eRPC."
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
  description = "Container image for rindexer."
  type        = string
  default     = "ghcr.io/joshstevens19/rindexer:latest"
}

variable "indexer_rpc_url" {
  description = "RPC endpoint URL for the indexer. If empty and rpc_proxy is enabled, auto-resolves to the eRPC internal service discovery URL."
  type        = string
  default     = ""
}

variable "indexer_storage_backend" {
  description = "Storage backend for rindexer: postgres (managed RDS) or clickhouse (BYODB)."
  type        = string
  default     = "postgres"
}

# --- ClickHouse BYODB ---

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
  default     = "rindexer"
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
  description = "Map of ABI filename to JSON content, e.g. { \"ERC20.json\" = file(\"abis/ERC20.json\") }. Uploaded to S3 alongside rindexer.yaml."
  type        = map(string)
  default     = {}
}

variable "indexer_instances" {
  description = "Multiple indexer instances with independent configs. Empty = single instance (backward compat)."
  type = list(object({
    name          = string
    config_key    = optional(string)
    node_role     = optional(string)
    workload_type = optional(string) # "deployment" (default) or "job" (one-shot backfill)
  }))
  default = []
}

variable "indexer_extra_env" {
  description = "Additional plaintext environment variables for the indexer container."
  type        = map(string)
  default     = {}
}

variable "indexer_extra_secret_env" {
  description = "Additional sensitive environment variables for the indexer container."
  type        = map(string)
  default     = {}
  sensitive   = true
}

# --- Custom Services ---

variable "custom_services" {
  description = "User-defined containerized services deployed alongside the indexer stack."
  type = list(object({
    name             = string
    image            = string
    port             = number
    health_path      = optional(string, "/health")
    replicas         = optional(number, 1)
    cpu_request      = optional(string, "250m")
    memory_request   = optional(string, "256Mi")
    cpu_limit        = optional(string, "500m")
    memory_limit     = optional(string, "512Mi")
    env              = optional(map(string), {})
    secret_env       = optional(map(string), {})
    ingress_hostname = optional(string)
    ingress_path     = optional(string, "/")
    node_role        = optional(string)
    tolerations = optional(list(object({
      key      = string
      operator = optional(string, "Equal")
      value    = optional(string)
      effect   = optional(string, "NoSchedule")
    })), [])
    enable_egress     = optional(bool, false)
    image_pull_policy = optional(string, "Always")
  }))
  default = []
}

# --- k3s ---

variable "k3s_version" {
  description = "k3s version to install."
  type        = string
  default     = "v1.30.4+k3s1"
}

variable "k3s_instance_type" {
  description = "EC2 instance type for k3s host."
  type        = string
  default     = "t3.small"
}

variable "k3s_api_allowed_cidrs" {
  description = "CIDR blocks allowed to access k3s API (port 6443). Defaults to VPC CIDR when empty."
  type        = list(string)
  default     = []
}

variable "k3s_worker_nodes" {
  description = "Worker nodes to join the k3s cluster. Each gets a dedicated EC2 instance. Set use_spot=true for interruptible workloads."
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
  description = "How secrets are delivered to workloads: inline, provider (AWS SM + ESO), or external (user-managed store)."
  type        = string
  default     = "inline"
}

variable "secrets_manager_secret_arn" {
  description = "ARN of a pre-existing AWS Secrets Manager secret (BYOA path). When set, skips SM secret creation."
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
  description = "Name of a user-managed ClusterSecretStore for secrets_mode=external."
  type        = string
  default     = ""
}

variable "external_secret_key" {
  description = "Secret key/name in the external store that holds workload env vars."
  type        = string
  default     = ""
}

variable "eso_chart_version" {
  description = "External Secrets Operator Helm chart version."
  type        = string
  default     = "0.9.13"
}

# --- Monitoring ---

variable "monitoring_enabled" {
  description = "Enable kube-prometheus-stack monitoring addon."
  type        = bool
  default     = false
}

variable "kube_prometheus_stack_version" {
  description = "kube-prometheus-stack Helm chart version."
  type        = string
  default     = "72.6.2"
}

variable "grafana_admin_password_secret_name" {
  description = "Existing K8s Secret name for Grafana admin password."
  type        = string
  default     = ""
}

variable "alertmanager_slack_webhook_secret_name" {
  description = "Existing K8s Secret name holding Slack webhook URL."
  type        = string
  default     = ""
}

variable "alertmanager_sns_topic_arn" {
  description = "SNS topic ARN for Alertmanager webhook routing."
  type        = string
  default     = ""
}

variable "alertmanager_pagerduty_routing_key_secret_name" {
  description = "Existing K8s Secret name holding PagerDuty routing key."
  type        = string
  default     = ""
}

variable "alertmanager_route_target" {
  description = "Alertmanager receiver target: slack, sns, or pagerduty."
  type        = string
  default     = "slack"
}

variable "alertmanager_slack_channel" {
  description = "Slack channel name for alert delivery."
  type        = string
  default     = "#alerts"
}

variable "loki_enabled" {
  description = "Deploy Loki + Promtail for log aggregation."
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
  description = "Enable PVC for Loki."
  type        = bool
  default     = false
}

variable "clickhouse_metrics_url" {
  description = "Optional BYO ClickHouse metrics endpoint."
  type        = string
  default     = ""
}

variable "grafana_ingress_enabled" {
  description = "Expose Grafana via Ingress."
  type        = bool
  default     = true
}

variable "grafana_hostname" {
  description = "Grafana hostname for Ingress."
  type        = string
  default     = ""
}

variable "grafana_extra_dashboards" {
  description = "Map of dashboard filename to JSON content for Grafana sidecar loading."
  type        = map(string)
  default     = {}
}

variable "ingress_class_name" {
  description = "Ingress class name for Grafana."
  type        = string
  default     = "nginx"
}
