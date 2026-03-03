variable "project_name" {
  type = string
}

variable "infrastructure_provider" {
  type    = string
  default = "aws"
}

variable "deployment_target" {
  type    = string
  default = "managed"
}

variable "runtime_arch" {
  type    = string
  default = "multi"
}

variable "database_mode" {
  type    = string
  default = "self_hosted"
}

variable "streaming_mode" {
  type    = string
  default = "disabled"
}

variable "ingress_mode" {
  type    = string
  default = "self_hosted"
}

variable "compute_engine" {
  type    = string
  default = "ec2"
}

variable "workload_mode" {
  type    = string
  default = "terraform"
}

variable "ssh_public_key" {
  type    = string
  default = ""
}

variable "ec2_instance_type" {
  type    = string
  default = "t3.small"
}

variable "aws_region" {
  type    = string
  default = "us-east-1"
}

variable "aws_skip_credentials_validation" {
  type    = bool
  default = false
}

variable "networking_enabled" {
  type    = bool
  default = true
}

variable "network_environment" {
  type    = string
  default = "dev"
}

variable "network_vpc_cidr" {
  type    = string
  default = "10.42.0.0/16"
}

variable "network_availability_zones" {
  type    = list(string)
  default = ["us-east-1a", "us-east-1b"]
}

variable "network_enable_nat_gateway" {
  type    = bool
  default = true
}

variable "network_enable_vpc_endpoints" {
  type    = bool
  default = true
}

# --- Postgres ---

variable "postgres_enabled" {
  type    = bool
  default = false
}

variable "postgres_instance_class" {
  type    = string
  default = "db.t4g.micro"
}

variable "postgres_engine_version" {
  type    = string
  default = "16.4"
}

variable "postgres_db_name" {
  type    = string
  default = "rindexer"
}

variable "postgres_db_username" {
  type    = string
  default = "rindexer"
}

variable "postgres_backup_retention" {
  type    = number
  default = 7
}

# --- RPC Proxy ---

variable "rpc_proxy_enabled" {
  type    = bool
  default = false
}

variable "rpc_proxy_image" {
  type    = string
  default = "ghcr.io/erpc/erpc:latest"
}

# --- Indexer ---

variable "indexer_enabled" {
  type    = bool
  default = true
}

variable "indexer_image" {
  type    = string
  default = "ghcr.io/joshstevens19/rindexer:latest"
}

variable "indexer_rpc_url" {
  type    = string
  default = ""
}

variable "indexer_storage_backend" {
  type    = string
  default = "postgres"
}
