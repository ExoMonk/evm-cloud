variable "project_name" {
  description = "Project name prefix for Helm release naming."
  type        = string
}

variable "namespace" {
  description = "Default Kubernetes namespace for addon deployments."
  type        = string
  default     = "addons"
}

variable "eso_enabled" {
  description = "Enable External Secrets Operator addon."
  type        = bool
  default     = false
}

variable "eso_chart_version" {
  description = "External Secrets Operator Helm chart version."
  type        = string
  default     = "0.9.13"
}
