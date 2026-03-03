variable "project_name" {
  description = "Project identifier."
  type        = string
}

variable "host_address" {
  description = "IP or hostname of the VPS."
  type        = string
}

variable "ssh_user" {
  description = "SSH user."
  type        = string
}

variable "ssh_private_key_path" {
  description = "Path to SSH private key file."
  type        = string
}

variable "ssh_port" {
  description = "SSH port."
  type        = number
  default     = 22
}

variable "enable_rpc_proxy" {
  description = "Enable eRPC proxy in Docker Compose."
  type        = bool
  default     = false
}

variable "enable_indexer" {
  description = "Enable rindexer in Docker Compose."
  type        = bool
  default     = false
}

variable "rpc_proxy_image" {
  description = "Container image for eRPC."
  type        = string
}

variable "indexer_image" {
  description = "Container image for rindexer."
  type        = string
}

variable "rpc_proxy_mem_limit" {
  description = "Docker memory limit for eRPC."
  type        = string
  default     = "1g"
}

variable "indexer_mem_limit" {
  description = "Docker memory limit for rindexer."
  type        = string
  default     = "2g"
}

variable "erpc_config_yaml" {
  description = "Full erpc.yaml content."
  type        = string
  default     = ""
}

variable "rindexer_config_yaml" {
  description = "Full rindexer.yaml content."
  type        = string
  default     = ""
}

variable "rindexer_abis" {
  description = "Map of ABI filename to JSON content."
  type        = map(string)
  default     = {}
}

# --- Ingress / TLS ---

variable "ingress_mode" {
  description = "Ingress mode: none, cloudflare, caddy."
  type        = string
  default     = "none"
}

variable "ingress_domain" {
  description = "Domain for TLS certificate and routing."
  type        = string
  default     = ""
}

variable "ingress_tls_email" {
  description = "Email for Let's Encrypt (caddy mode)."
  type        = string
  default     = ""
}

variable "ingress_cloudflare_origin_cert" {
  description = "Cloudflare Origin Certificate (PEM)."
  type        = string
  default     = ""
  sensitive   = true
}

variable "ingress_cloudflare_origin_key" {
  description = "Cloudflare Origin Certificate private key (PEM)."
  type        = string
  default     = ""
  sensitive   = true
}

variable "ingress_caddy_image" {
  description = "Container image for Caddy reverse proxy."
  type        = string
  default     = "caddy:2.9.1-alpine"
}

variable "ingress_caddy_mem_limit" {
  description = "Docker memory limit for Caddy container."
  type        = string
  default     = "128m"
}

variable "ingress_request_body_max_size" {
  description = "Maximum request body size for ingress."
  type        = string
  default     = "1m"
}

variable "ingress_tls_staging" {
  description = "Use Let's Encrypt staging ACME server."
  type        = bool
  default     = false
}

variable "ingress_hsts_preload" {
  description = "Add 'preload' to HSTS header."
  type        = bool
  default     = false
}

variable "secret_payload" {
  description = "Map of secret env vars (RPC_URL, CLICKHOUSE_*, DATABASE_URL)."
  type        = map(string)
  default     = {}
  sensitive   = true
}
