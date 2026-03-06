variable "project_name" {
  type    = string
  default = "evm-cloud-bm-k3s"
}

# --- Bare Metal SSH ---

variable "bare_metal_host" {
  description = "IP or hostname of your VPS"
  type        = string
}

variable "bare_metal_ssh_user" {
  description = "SSH user for the VPS"
  type        = string
  default     = "ubuntu"
}

variable "ssh_private_key_path" {
  description = "Path to SSH private key file used for provisioning and config updates."
  type        = string
  sensitive   = true
}

variable "bare_metal_ssh_port" {
  description = "SSH port"
  type        = number
  default     = 22
}

# --- k3s ---

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

# --- PostgreSQL BYODB ---

variable "indexer_postgres_url" {
  description = "PostgreSQL connection string (e.g. postgres://user:pass@host:5432/db)"
  type        = string
  sensitive   = true
}
