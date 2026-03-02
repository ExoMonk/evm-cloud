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

variable "secret_payload" {
  description = "Map of secret env vars (RPC_URL, CLICKHOUSE_*, DATABASE_URL)."
  type        = map(string)
  default     = {}
  sensitive   = true
}
