variable "project_name" {
  description = "Project name used for resource tags and naming."
  type        = string
}

variable "environment" {
  description = "Networking environment profile."
  type        = string

  validation {
    condition     = contains(["dev", "production", "platform"], var.environment)
    error_message = "environment must be one of: dev, production, platform."
  }
}

variable "vpc_cidr" {
  description = "CIDR block for VPC."
  type        = string
}

variable "availability_zones" {
  description = "Availability zones used to create subnets."
  type        = list(string)

  validation {
    condition     = length(var.availability_zones) >= 1
    error_message = "availability_zones must contain at least one AZ."
  }
}

variable "enable_nat_gateway" {
  description = "Enable NAT gateway for private subnet egress."
  type        = bool
}

variable "enable_vpc_endpoints" {
  description = "Enable baseline VPC endpoints (S3 + interface endpoints)."
  type        = bool
}
