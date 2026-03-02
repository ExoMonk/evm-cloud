variable "project_name" {
  type    = string
  default = "evm-cloud-bare-metal"
}

variable "workload_mode" {
  type    = string
  default = "terraform"
}

# --- Bare Metal ---

variable "bare_metal_host" {
  type = string
}

variable "bare_metal_ssh_user" {
  type    = string
  default = "ubuntu"
}

variable "bare_metal_ssh_private_key_path" {
  type = string
}

variable "bare_metal_ssh_port" {
  type    = number
  default = 22
}

variable "bare_metal_rpc_proxy_mem_limit" {
  type    = string
  default = "1g"
}

variable "bare_metal_indexer_mem_limit" {
  type    = string
  default = "2g"
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
  type = string
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
