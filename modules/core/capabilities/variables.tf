variable "infrastructure_provider" {
  description = "Selected infrastructure provider adapter."
  type        = string
}

variable "deployment_target" {
  description = "Deployment posture: managed, hybrid, self_hosted."
  type        = string
}

variable "runtime_arch" {
  description = "Runtime architecture intent."
  type        = string
}

variable "database_mode" {
  description = "Database mode."
  type        = string
}

variable "streaming_mode" {
  description = "Streaming mode."
  type        = string
}

variable "ingress_mode" {
  description = "Ingress mode."
  type        = string
}
