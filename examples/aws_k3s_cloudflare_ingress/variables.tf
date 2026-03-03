variable "project_name" {
  type    = string
  default = "evm-cloud-k3s-cf"
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
  type      = string
  sensitive = true
}

variable "k3s_ssh_private_key_path" {
  type      = string
  sensitive = true
}

variable "k3s_api_allowed_cidrs" {
  type = list(string)
}

# --- k3s ---

variable "k3s_instance_type" {
  type    = string
  default = "t3.medium"
}

variable "k3s_version" {
  type    = string
  default = "v1.30.4+k3s1"
}

# --- Ingress (Cloudflare) ---

variable "ingress_domain" {
  description = "Domain for Cloudflare TLS (e.g. rpc.example.com)"
  type        = string
}

variable "ingress_cloudflare_origin_cert" {
  description = "Cloudflare origin certificate (PEM)"
  type        = string
  sensitive   = true
}

variable "ingress_cloudflare_origin_key" {
  description = "Cloudflare origin certificate private key (PEM)"
  type        = string
  sensitive   = true
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
  type    = string
  default = ""
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
