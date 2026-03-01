variable "project_name" {
  description = "Project name used for resource naming."
  type        = string
}

variable "environment" {
  description = "Deployment environment (dev, production, platform)."
  type        = string
}

variable "subnet_ids" {
  description = "Private subnet IDs for ECS service networking."
  type        = list(string)
}

variable "security_group_id" {
  description = "Security group ID for indexer access (from networking module)."
  type        = string
}

variable "cluster_arn" {
  description = "ARN of the shared ECS cluster."
  type        = string
}

variable "image" {
  description = "Container image for rindexer. Override for multi-arch compatibility."
  type        = string
  default     = "ghcr.io/joshstevens19/rindexer:latest"
}

variable "cpu" {
  description = "CPU units for the rindexer task (1024 = 1 vCPU)."
  type        = number
  default     = 1024
}

variable "memory" {
  description = "Memory in MiB for the rindexer task."
  type        = number
  default     = 2048
}

variable "storage_backend" {
  description = "Storage backend for rindexer: postgres (managed RDS) or clickhouse (BYODB)."
  type        = string
  default     = "postgres"

  validation {
    condition     = contains(["postgres", "clickhouse"], var.storage_backend)
    error_message = "storage_backend must be one of: postgres, clickhouse."
  }
}

# --- Postgres (managed RDS) ---

variable "db_secret_arn" {
  description = "ARN of the Secrets Manager secret containing database credentials. Required when storage_backend=postgres."
  type        = string
  default     = ""
}

variable "db_host" {
  description = "Database host address. Required when storage_backend=postgres."
  type        = string
  default     = ""
}

variable "db_port" {
  description = "Database port."
  type        = number
  default     = 5432
}

variable "db_name" {
  description = "Database name."
  type        = string
  default     = "rindexer"
}

# --- ClickHouse (BYODB) ---

variable "clickhouse_url" {
  description = "ClickHouse HTTP endpoint (e.g. http://clickhouse.example.com:8123). Required when storage_backend=clickhouse."
  type        = string
  default     = ""
}

variable "clickhouse_user" {
  description = "ClickHouse username. Required when storage_backend=clickhouse."
  type        = string
  default     = "default"
}

variable "clickhouse_password" {
  description = "ClickHouse password. Required when storage_backend=clickhouse."
  type        = string
  default     = ""
  sensitive   = true
}

variable "clickhouse_db" {
  description = "ClickHouse database name. Required when storage_backend=clickhouse."
  type        = string
  default     = "default"
}

# --- Common ---

variable "rpc_url" {
  description = "RPC endpoint URL. Injected as RPC_URL — reference as $${RPC_URL} in rindexer.yaml networks section."
  type        = string
}

variable "config_bucket_name" {
  description = "S3 bucket name containing rindexer config files."
  type        = string
}

variable "config_object_prefix" {
  description = "S3 object key prefix for rindexer config (rindexer.yaml and abis/ live under this prefix)."
  type        = string
}

variable "aws_region" {
  description = "AWS region for CloudWatch log group and S3 access."
  type        = string
}

variable "task_execution_role_arn" {
  description = "Pre-provisioned ECS task execution role ARN."
  type        = string
}

variable "task_role_arn" {
  description = "Pre-provisioned ECS task role ARN for indexer runtime permissions."
  type        = string
}
