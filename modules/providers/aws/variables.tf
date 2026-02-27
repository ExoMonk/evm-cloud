variable "project_name" {
  description = "Project identifier used for naming resources."
  type        = string
}

variable "deployment_target" {
  description = "Deployment posture selected at root."
  type        = string
}

variable "runtime_arch" {
  description = "Runtime architecture intent selected at root."
  type        = string
}

variable "database_mode" {
  description = "Database mode selected at root."
  type        = string
}

variable "streaming_mode" {
  description = "Streaming mode selected at root."
  type        = string
}

variable "ingress_mode" {
  description = "Ingress mode selected at root."
  type        = string
}

variable "aws_region" {
  description = "AWS region used by adapter resources."
  type        = string
}

variable "networking_enabled" {
  description = "Enable networking module provisioning."
  type        = bool
}

variable "network_environment" {
  description = "Networking profile environment."
  type        = string
}

variable "network_vpc_cidr" {
  description = "VPC CIDR for networking module."
  type        = string
}

variable "network_availability_zones" {
  description = "Availability zones for networking module."
  type        = list(string)
}

variable "network_enable_nat_gateway" {
  description = "Enable NAT gateway in networking module."
  type        = bool
}

variable "network_enable_vpc_endpoints" {
  description = "Enable VPC endpoints in networking module."
  type        = bool
}
