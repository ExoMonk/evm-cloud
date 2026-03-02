variable "project_name" {
  type    = string
  default = "evm-cloud-k3s"
}

variable "aws_region" {
  type    = string
  default = "us-east-1"
}

variable "aws_skip_credentials_validation" {
  type    = bool
  default = false
}

variable "network_vpc_cidr" {
  type    = string
  default = "10.42.0.0/16"
}

variable "network_availability_zones" {
  type    = list(string)
  default = ["us-east-1a", "us-east-1b"]
}

# --- SSH Keys ---

variable "ssh_public_key" {
  description = "SSH public key content for EC2 instance"
  type        = string
  sensitive   = true
}

variable "k3s_ssh_private_key_path" {
  description = "Path to SSH private key for k3s provisioner (e.g., ~/.ssh/id_ed25519)"
  type        = string
  sensitive   = true
}

variable "k3s_api_allowed_cidrs" {
  description = "CIDRs allowed to reach SSH (22) and k3s API (6443). Must include your IP for provisioning. Example: [\"203.0.113.42/32\"]"
  type        = list(string)
}

# --- k3s ---

variable "k3s_instance_type" {
  description = "EC2 instance type for k3s host"
  type        = string
  default     = "t3.medium"
}

variable "k3s_version" {
  description = "k3s version to install"
  type        = string
  default     = "v1.30.4+k3s1"
}

# --- RPC Proxy ---

variable "rpc_proxy_enabled" {
  type    = bool
  default = true
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
  description = "RPC URL for the indexer. Leave empty to auto-resolve to eRPC when rpc_proxy_enabled=true."
  type        = string
  default     = ""
}

# --- ClickHouse BYODB ---

variable "indexer_clickhouse_url" {
  type      = string
  sensitive = true
}

variable "indexer_clickhouse_user" {
  type    = string
  default = "default"
}

variable "indexer_clickhouse_password" {
  type      = string
  sensitive = true
}

variable "indexer_clickhouse_db" {
  type    = string
  default = "default"
}
