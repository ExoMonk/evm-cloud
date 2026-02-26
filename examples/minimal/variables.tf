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
