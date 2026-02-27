variable "project_name" {
  type = string
}

variable "infrastructure_provider" {
  type    = string
  default = "aws"
}

variable "deployment_target" {
  type    = string
  default = "managed"
}

variable "runtime_arch" {
  type    = string
  default = "multi"
}

variable "database_mode" {
  type    = string
  default = "self_hosted"
}

variable "streaming_mode" {
  type    = string
  default = "disabled"
}

variable "ingress_mode" {
  type    = string
  default = "self_hosted"
}

variable "aws_region" {
  type    = string
  default = "us-east-1"
}

variable "aws_skip_credentials_validation" {
  type    = bool
  default = false
}

variable "networking_enabled" {
  type    = bool
  default = false
}

variable "network_environment" {
  type    = string
  default = "dev"
}

variable "network_vpc_cidr" {
  type    = string
  default = "10.42.0.0/16"
}

variable "network_availability_zones" {
  type    = list(string)
  default = ["us-east-1a", "us-east-1b"]
}

variable "network_enable_nat_gateway" {
  type    = bool
  default = false
}

variable "network_enable_vpc_endpoints" {
  type    = bool
  default = false
}
