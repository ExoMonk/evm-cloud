variable "project_name" {
  type    = string
  default = "defi-swaps"
}

# --- Bare Metal SSH ---

variable "bare_metal_host" {
  description = "IP or hostname of your VPS"
  type        = string
}

variable "bare_metal_ssh_user" {
  description = "SSH user on the VPS"
  type        = string
  default     = "ubuntu"
}

variable "ssh_private_key_path" {
  description = "Path to SSH private key file"
  type        = string
  sensitive   = true
}

variable "bare_metal_ssh_port" {
  description = "SSH port"
  type        = number
  default     = 22
}

# --- Ingress / Cloudflare ---

variable "erpc_hostname" {
  description = "Public hostname for eRPC (e.g. rpc.example.com). Optional — only needed if you want to expose eRPC publicly."
  type        = string
  default     = ""
}

variable "api_hostname" {
  description = "Public hostname for the swap-api (e.g. api.example.com). Required when ingress_mode != none."
  type        = string
  default     = ""
}

variable "ingress_cloudflare_origin_cert" {
  description = "Cloudflare origin certificate (PEM). Required when ingress_mode = cloudflare."
  type        = string
  default     = ""
  sensitive   = true
}

variable "ingress_cloudflare_origin_key" {
  description = "Cloudflare origin certificate private key (PEM). Required when ingress_mode = cloudflare."
  type        = string
  default     = ""
  sensitive   = true
}

# --- k3s ---

variable "k3s_version" {
  type    = string
  default = "v1.30.4+k3s1"
}

# --- Images ---

variable "rpc_proxy_image" {
  type    = string
  default = "ghcr.io/erpc/erpc:latest"
}

variable "indexer_image" {
  type    = string
  default = "ghcr.io/joshstevens19/rindexer:latest"
}

variable "swap_api_image" {
  description = "Container image for the Swap API custom service. Default works with scripts/build-and-ship.sh (local image imported into k3s)."
  type        = string
  default     = "docker.io/library/swap-api:local"
}

# --- Webhook / Whale Alerts ---

variable "webhook_secret" {
  description = "Shared secret between rindexer webhook stream and swap-api"
  type        = string
  sensitive   = true
}

variable "whale_threshold" {
  description = "Absolute token amount (raw units) above which a swap triggers a whale alert. Default: 1e18 (1 token with 18 decimals)"
  type        = string
  default     = "1000000000000000000"
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
  default = "rindexer"
}
