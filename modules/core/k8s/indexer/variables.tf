variable "project_name" {
  description = "Project name used for resource naming."
  type        = string
}

variable "namespace" {
  description = "Kubernetes namespace for indexer resources."
  type        = string
  default     = "default"
}

variable "image" {
  description = "Container image for rindexer."
  type        = string
  default     = "ghcr.io/joshstevens19/rindexer:latest"
}

variable "storage_backend" {
  description = "Storage backend: postgres or clickhouse."
  type        = string
  default     = "postgres"

  validation {
    condition     = contains(["postgres", "clickhouse"], var.storage_backend)
    error_message = "storage_backend must be one of: postgres, clickhouse."
  }
}

variable "rpc_url" {
  description = "RPC endpoint URL, injected as RPC_URL env var."
  type        = string
}

variable "rindexer_config_yaml" {
  description = "Full rindexer.yaml content, injected into a ConfigMap."
  type        = string
}

variable "rindexer_abis" {
  description = "Map of ABI filename to JSON content."
  type        = map(string)
  default     = {}
}

# --- Postgres ---

variable "database_url" {
  description = "Pre-composed DATABASE_URL for postgres backend. Injected into a K8s Secret."
  type        = string
  default     = ""
  sensitive   = true
}

# --- ClickHouse BYODB ---

variable "clickhouse_url" {
  description = "ClickHouse HTTP endpoint."
  type        = string
  default     = ""
}

variable "clickhouse_user" {
  description = "ClickHouse username."
  type        = string
  default     = "default"
}

variable "clickhouse_password" {
  description = "ClickHouse password."
  type        = string
  default     = ""
  sensitive   = true
}

variable "clickhouse_db" {
  description = "ClickHouse database name."
  type        = string
  default     = "default"
}

# --- Resource sizing ---

variable "cpu_request" {
  description = "CPU request for the indexer container."
  type        = string
  default     = "512m"
}

variable "memory_request" {
  description = "Memory request for the indexer container."
  type        = string
  default     = "1Gi"
}

variable "cpu_limit" {
  description = "CPU limit for the indexer container."
  type        = string
  default     = "1"
}

variable "memory_limit" {
  description = "Memory limit for the indexer container."
  type        = string
  default     = "2Gi"
}

variable "monitoring_enabled" {
  description = "Whether monitoring stack is enabled (controls ServiceMonitor creation)."
  type        = bool
  default     = false
}

variable "wait_for_rollout" {
  description = "Wait for the Deployment rollout to complete."
  type        = bool
  default     = true
}

variable "extra_env" {
  description = "Additional environment variables to inject into the indexer container."
  type        = map(string)
  default     = {}
}
