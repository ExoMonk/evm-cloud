variable "project_name" {
  description = "Project identifier used for naming resources."
  type        = string

  validation {
    condition     = length(trimspace(var.project_name)) > 0
    error_message = "project_name must be a non-empty string."
  }
}

variable "infrastructure_provider" {
  description = "Provider adapter to use. Currently implemented: aws."
  type        = string
  default     = "aws"
}

variable "deployment_target" {
  description = "High-level deployment mode."
  type        = string
  default     = "managed"

  validation {
    condition     = contains(["managed", "hybrid", "self_hosted"], var.deployment_target)
    error_message = "deployment_target must be one of: managed, hybrid, self_hosted."
  }
}

variable "runtime_arch" {
  description = "Runtime architecture intent for workloads."
  type        = string
  default     = "multi"

  validation {
    condition     = contains(["amd64", "arm64", "multi"], var.runtime_arch)
    error_message = "runtime_arch must be one of: amd64, arm64, multi."
  }
}

variable "database_mode" {
  description = "Database operating mode."
  type        = string
  default     = "managed"

  validation {
    condition     = contains(["managed", "self_hosted"], var.database_mode)
    error_message = "database_mode must be one of: managed, self_hosted."
  }
}

variable "streaming_mode" {
  description = "Streaming operating mode."
  type        = string
  default     = "disabled"

  validation {
    condition     = contains(["managed", "self_hosted", "disabled"], var.streaming_mode)
    error_message = "streaming_mode must be one of: managed, self_hosted, disabled."
  }
}

variable "ingress_mode" {
  description = "Ingress operating mode."
  type        = string
  default     = "managed_lb"

  validation {
    condition     = contains(["managed_lb", "self_hosted"], var.ingress_mode)
    error_message = "ingress_mode must be one of: managed_lb, self_hosted."
  }
}

variable "networking_enabled" {
  description = "Enable AWS networking module provisioning in the provider adapter."
  type        = bool
  default     = false
}

variable "network_environment" {
  description = "Networking profile environment."
  type        = string
  default     = "dev"

  validation {
    condition     = contains(["dev", "production", "platform"], var.network_environment)
    error_message = "network_environment must be one of: dev, production, platform."
  }
}

variable "aws_region" {
  description = "AWS region for provider-backed resources."
  type        = string
  default     = "us-east-1"
}

variable "aws_skip_credentials_validation" {
  description = "Skip AWS provider credential/account validation checks (useful for local simulation)."
  type        = bool
  default     = false
}

variable "network_vpc_cidr" {
  description = "VPC CIDR block for networking module."
  type        = string
  default     = "10.42.0.0/16"
}

variable "network_availability_zones" {
  description = "Availability zones used for subnets."
  type        = list(string)
  default     = ["us-east-1a", "us-east-1b"]
}

variable "network_enable_nat_gateway" {
  description = "Enable NAT gateway for private subnet egress."
  type        = bool
  default     = false
}

variable "network_enable_vpc_endpoints" {
  description = "Enable baseline VPC endpoints (S3 + interface endpoints)."
  type        = bool
  default     = false
}
