variable "project_name" {
  description = "Project identifier used for naming resources."
  type        = string
}

variable "environment" {
  description = "Environment name (dev, production)."
  type        = string
}

variable "subnet_id" {
  description = "Public subnet ID for the EC2 instance."
  type        = string
}

variable "security_group_id" {
  description = "Security group ID for the EC2 instance."
  type        = string
}

variable "instance_profile_name" {
  description = "IAM instance profile name for the EC2 instance."
  type        = string
}

variable "ssh_public_key" {
  description = "SSH public key for the deploy key pair."
  type        = string
}

variable "instance_type" {
  description = "EC2 instance type."
  type        = string
  default     = "t3.medium"
}

variable "root_volume_size" {
  description = "Root EBS volume size in GB."
  type        = number
  default     = 30
}

variable "enable_rpc_proxy" {
  description = "Deploy eRPC proxy container."
  type        = bool
  default     = false
}

variable "enable_indexer" {
  description = "Deploy rindexer indexer container."
  type        = bool
  default     = false
}

variable "rpc_proxy_image" {
  description = "Container image for eRPC."
  type        = string
  default     = "ghcr.io/erpc/erpc:latest"
}

variable "indexer_image" {
  description = "Container image for rindexer."
  type        = string
  default     = "ghcr.io/joshstevens19/rindexer:latest"
}

variable "rpc_url" {
  description = "RPC endpoint URL for the indexer. When eRPC is enabled, auto-set to http://erpc:4000."
  type        = string
  default     = ""
}

variable "erpc_yaml_content" {
  description = "Full erpc.yaml content."
  type        = string
  default     = ""
}

variable "rindexer_yaml_content" {
  description = "Full rindexer.yaml content."
  type        = string
  default     = ""
}

variable "abi_files" {
  description = "Map of ABI filename to JSON content."
  type        = map(string)
  default     = {}
}

variable "aws_region" {
  description = "AWS region."
  type        = string
}

variable "tags" {
  description = "Common tags for all resources."
  type        = map(string)
  default     = {}
}

variable "storage_backend" {
  description = "Storage backend: postgres or clickhouse."
  type        = string
  default     = "postgres"
}

# --- Postgres ---

variable "db_host" {
  description = "Postgres host."
  type        = string
  default     = ""
}

variable "db_port" {
  description = "Postgres port."
  type        = number
  default     = 5432
}

variable "db_name" {
  description = "Postgres database name."
  type        = string
  default     = "rindexer"
}

variable "db_username" {
  description = "Postgres username."
  type        = string
  default     = ""
}

variable "db_password" {
  description = "Postgres password."
  type        = string
  default     = ""
  sensitive   = true
}

# --- ClickHouse ---

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

variable "rpc_proxy_mem_limit" {
  description = "Docker memory limit for eRPC container (e.g. 512m, 1g, 2g)."
  type        = string
  default     = "1g"
}

variable "indexer_mem_limit" {
  description = "Docker memory limit for rindexer container (e.g. 1g, 2g, 4g)."
  type        = string
  default     = "2g"
}

variable "workload_mode" {
  description = "Workload ownership: terraform (full cloud-init) or external (Docker-ready instance, no workloads)."
  type        = string
  default     = "terraform"
}

variable "secret_name_prefix" {
  description = "Prefix for Secrets Manager secret name."
  type        = string
  default     = "evm-cloud"
}
