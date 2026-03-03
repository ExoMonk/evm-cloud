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
    name       = string
    config_key = optional(string)
    node_role  = optional(string)
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
  default     = "default"
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

