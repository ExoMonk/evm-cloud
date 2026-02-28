variable "project_name" {
  description = "Project identifier used for naming resources."
  type        = string

  validation {
    condition     = length(trimspace(var.project_name)) > 0
    error_message = "project_name must be a non-empty string."
  }
}

variable "infrastructure_provider" {
  description = "Provider adapter to use. Currently implemented: aws."
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
  description = "Ingress operating mode."
  type        = string
  default     = "managed_lb"

  validation {
    condition     = contains(["managed_lb", "self_hosted"], var.ingress_mode)
    error_message = "ingress_mode must be one of: managed_lb, self_hosted."
  }
}

variable "compute_engine" {
  description = "Compute engine for workloads: ecs (Fargate) or eks (Kubernetes). Changing on an existing deployment destroys and recreates all compute resources; database is preserved."
  type        = string
  default     = "ecs"

  validation {
    condition     = contains(["ecs", "eks"], var.compute_engine)
    error_message = "compute_engine must be one of: ecs, eks."
  }
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
