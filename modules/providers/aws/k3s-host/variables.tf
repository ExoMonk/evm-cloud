variable "project_name" {
  description = "Project identifier used for naming resources."
  type        = string
}

variable "environment" {
  description = "Environment name (dev, production, platform)."
  type        = string
}

variable "instance_type" {
  description = "EC2 instance type for k3s host."
  type        = string
  default     = "t3.small"
}

variable "subnet_id" {
  description = "Subnet ID to place the k3s host in."
  type        = string
}

variable "vpc_id" {
  description = "VPC ID for security group creation."
  type        = string
}

variable "vpc_cidr" {
  description = "VPC CIDR block, used as default for k3s API access restriction."
  type        = string
}

variable "ssh_public_key" {
  description = "SSH public key for the deploy key pair."
  type        = string
  sensitive   = true
}

variable "k3s_api_allowed_cidrs" {
  description = "CIDR blocks allowed to access k3s API (port 6443). Defaults to VPC CIDR when empty."
  type        = list(string)
  default     = []
}

variable "use_spot" {
  description = "Use a spot instance instead of on-demand. Only recommended for interruptible workloads (e.g. backfill workers)."
  type        = bool
  default     = false
}

variable "additional_security_group_ids" {
  description = "Additional security group IDs to attach (e.g. indexer SG for DB access)."
  type        = list(string)
  default     = []
}

variable "tags" {
  description = "Common resource tags."
  type        = map(string)
  default     = {}
}

variable "secrets_mode" {
  description = "Secrets delivery mode. When 'provider', creates IAM instance profile for Secrets Manager access."
  type        = string
  default     = "inline"
}

variable "secrets_manager_prefix" {
  description = "SM secret name prefix for IAM policy (e.g. evm-cloud/myproject). Used as wildcard fallback when secret_arn is not set."
  type        = string
  default     = ""
}

variable "secrets_manager_secret_arn" {
  description = "Exact ARN of the SM secret (BYOA or Terraform-created). When set, IAM policy targets this ARN directly instead of prefix wildcard."
  type        = string
  default     = ""
}

