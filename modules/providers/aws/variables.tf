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

variable "aws_region" {
  description = "AWS region used by adapter resources."
  type        = string
}

variable "compute_engine" {
  description = "Compute engine for workloads: ecs (Fargate) or eks (Kubernetes)."
  type        = string
  default     = "ecs"

  validation {
    condition     = contains(["ecs", "eks"], var.compute_engine)
    error_message = "compute_engine must be one of: ecs, eks."
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
  description = "RPC endpoint URL for the indexer. If empty and rpc_proxy is enabled, must be set to the eRPC proxy URL (no automatic service discovery in Tier 0)."
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

# --- Config injection (S3-backed) ---

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
