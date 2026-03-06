variable "project_name" {
  description = "Project name for E2E test infrastructure"
  type        = string
  default     = "evm-cloud-e2e"
}

variable "aws_region" {
  type    = string
  default = "us-east-1"
}

# --- SSH Keys ---

variable "ssh_public_key" {
  description = "SSH public key content for EC2 instance"
  type        = string
  sensitive   = true
}

variable "ssh_private_key_path" {
  description = "Path to SSH private key file used for provisioning and config updates."
  type        = string
  sensitive   = true
}

variable "k3s_api_allowed_cidrs" {
  description = "CIDRs allowed to reach SSH (22) and k3s API (6443). Must include your IP."
  type        = list(string)
}

# --- k3s ---

variable "k3s_instance_type" {
  description = "EC2 instance type for k3s host. t3.small = 2 vCPU, 2GB (~$15/mo)"
  type        = string
  default     = "t3.small"
}

variable "k3s_version" {
  description = "k3s version to install (pinned for test reproducibility)"
  type        = string
  default     = "v1.30.4+k3s1"
}
