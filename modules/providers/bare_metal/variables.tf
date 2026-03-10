variable "project_name" {
  description = "Project identifier used for naming resources."
  type        = string
}

variable "compute_engine" {
  description = "Compute engine: docker_compose or k3s."
  type        = string

  validation {
    condition     = contains(["docker_compose", "k3s"], var.compute_engine)
    error_message = "bare_metal compute_engine must be one of: docker_compose, k3s."
  }
}

variable "workload_mode" {
  description = "Workload ownership: terraform manages app resources, external delegates to CI/GitOps tools."
  type        = string
  default     = "terraform"
}

# --- SSH connection ---

variable "host_address" {
  description = "IP or hostname of the VPS."
  type        = string
}

variable "ssh_user" {
  description = "SSH user for the VPS."
  type        = string
  default     = "ubuntu"
}

variable "ssh_private_key_path" {
  description = "Path to SSH private key file."
  type        = string
}

variable "ssh_port" {
  description = "SSH port."
  type        = number
  default     = 22
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

variable "rpc_proxy_mem_limit" {
  description = "Docker memory limit for eRPC container."
  type        = string
  default     = "1g"
}

variable "erpc_config_yaml" {
  description = "Full erpc.yaml content."
  type        = string
  default     = ""
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
  description = "RPC endpoint URL for the indexer."
  type        = string
  default     = ""
}

variable "indexer_storage_backend" {
  description = "Storage backend for rindexer: postgres or clickhouse."
  type        = string
  default     = "clickhouse"
}

variable "indexer_mem_limit" {
  description = "Docker memory limit for rindexer container."
  type        = string
  default     = "2g"
}

variable "rindexer_config_yaml" {
  description = "Full rindexer.yaml content."
  type        = string
  default     = ""
}

variable "rindexer_abis" {
  description = "Map of ABI filename to JSON content."
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
    enable_egress = optional(bool, false)
  }))
  default = []
}

# --- PostgreSQL BYODB ---

variable "indexer_postgres_url" {
  description = "PostgreSQL connection string (e.g. postgres://user:pass@host:5432/db)."
  type        = string
  default     = ""
  sensitive   = true
}

# --- ClickHouse BYODB ---

variable "indexer_clickhouse_url" {
  description = "ClickHouse HTTP endpoint."
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

# --- k3s ---

variable "k3s_version" {
  description = "k3s version to install."
  type        = string
  default     = "v1.30.4+k3s1"
}

variable "k3s_worker_nodes" {
  description = "Worker nodes to join the k3s cluster. Each node must have a host address and be SSH-reachable from the Terraform runner."
  type = list(object({
    name                 = string
    host                 = optional(string)
    ssh_user             = optional(string)
    ssh_private_key_path = optional(string)
    ssh_port             = optional(number, 22)
    role                 = optional(string, "general")
    instance_type        = optional(string)
    use_spot             = optional(bool, false)
  }))
  default = []
}

# --- Ingress / TLS ---

variable "ingress_mode" {
  description = "Ingress mode selected at root."
  type        = string
  default     = "none"
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

# --- Secrets Management ---

variable "secrets_mode" {
  description = "How secrets are delivered to workloads: inline, provider (AWS-only), or external (user-managed store)."
  type        = string
  default     = "inline"
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

variable "ingress_class_name" {
  description = "Ingress class name for Grafana."
  type        = string
  default     = "nginx"
}

