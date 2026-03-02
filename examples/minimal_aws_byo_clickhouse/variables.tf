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
  type      = string
  default   = ""
  sensitive = true
}

variable "ec2_instance_type" {
  type    = string
  default = "t2.micro"
}

variable "ec2_rpc_proxy_mem_limit" {
  type    = string
  default = "256m"
}

variable "ec2_indexer_mem_limit" {
  type    = string
  default = "512m"
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
  default = false
}

variable "indexer_image" {
  type    = string
  default = "ghcr.io/joshstevens19/rindexer:latest"
}

variable "indexer_rpc_url" {
  type    = string
  default = ""
}

# --- ClickHouse BYODB ---

variable "indexer_clickhouse_url" {
  type      = string
  default   = ""
  sensitive = true
}

variable "indexer_clickhouse_user" {
  type    = string
  default = "default"
}

variable "indexer_clickhouse_password" {
  type      = string
  default   = ""
  sensitive = true
}

variable "indexer_clickhouse_db" {
  type    = string
  default = "default"
}
