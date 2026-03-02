variable "project_name" {
  description = "Project name prefix for Helm release naming."
  type        = string
}

variable "namespace" {
  description = "Default Kubernetes namespace for addon deployments."
  type        = string
  default     = "addons"
}
