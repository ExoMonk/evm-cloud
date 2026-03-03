variable "worker_nodes" {
  description = "Worker nodes to join the k3s cluster."
  type = list(object({
    name                 = string
    host                 = string
    ssh_user             = string
    ssh_private_key_path = string
    ssh_port             = optional(number, 22)
    role                 = optional(string, "general")
  }))

  validation {
    condition     = alltrue([for n in var.worker_nodes : contains(["indexer", "database", "evm-node", "monitoring", "general"], n.role)])
    error_message = "role must be one of: indexer, database, evm-node, monitoring, general."
  }
}

variable "server_host" {
  description = "k3s server host address (IP or hostname)."
  type        = string
}

variable "server_ssh_user" {
  description = "SSH user for the k3s server."
  type        = string
  default     = "ubuntu"
}

variable "server_ssh_private_key_path" {
  description = "Path to SSH private key for the k3s server."
  type        = string
}

variable "server_ssh_port" {
  description = "SSH port for the k3s server."
  type        = number
  default     = 22
}

variable "node_token" {
  description = "k3s server node token for agent authentication. Must match the server's token."
  type        = string
  sensitive   = true
}

variable "k3s_version" {
  description = "k3s version to install. Must match the server version."
  type        = string
}

variable "project_name" {
  description = "Project name used for k3s node naming."
  type        = string
}
