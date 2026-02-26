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
