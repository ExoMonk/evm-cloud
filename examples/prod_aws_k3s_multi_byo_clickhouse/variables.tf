variable "project_name" {
  type    = string
  default = "evm-cloud-k3s-multi"
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

variable "k3s_worker_nodes" {
  description = "Worker nodes to join the k3s cluster. Each gets a dedicated EC2 instance with the specified role label. Set use_spot=true for interruptible workloads."
  type = list(object({
    name          = string
    role          = optional(string, "general")
    instance_type = optional(string, "t3.medium")
    use_spot      = optional(bool, false)
    host          = optional(string)
  }))
  default = [
    { name = "backfill", role = "indexer", instance_type = "t3.medium", use_spot = true },
  ]
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

# --- Secrets Management ---

variable "secrets_mode" {
  description = "How secrets reach workloads: inline (passwords in handoff), provider (AWS SM + ESO), or external (user-managed store)."
  type        = string
  default     = "provider"
}

variable "secrets_manager_secret_arn" {
  description = "ARN of a pre-existing Secrets Manager secret. When set, skips secret creation (BYOA)."
  type        = string
  default     = ""
  sensitive   = true
}

variable "secrets_manager_kms_key_id" {
  description = "KMS key ID or alias for Secrets Manager encryption. Omit for AWS default key."
  type        = string
  default     = ""
}

variable "ec2_secret_recovery_window_in_days" {
  description = "SM secret deletion recovery window. 0 = immediate (dev/test), 7-30 for production."
  type        = number
  default     = 0
}

# --- Multi-instance indexer ---

variable "indexer_instances" {
  description = "Indexer instances. Each becomes a separate Helm release with optional per-instance config."
  type = list(object({
    name       = string
    config_key = optional(string)
    node_role  = optional(string)
  }))
  default = [
    { name = "indexer" },                                                  # live: runs on server, uses config/rindexer.yaml
    { name = "backfill", config_key = "backfill", node_role = "indexer" }, # backfill: runs on worker, uses config/backfill/rindexer.yaml
  ]
}
