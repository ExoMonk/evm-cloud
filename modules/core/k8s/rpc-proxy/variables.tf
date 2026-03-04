variable "project_name" {
  description = "Project name used for resource naming."
  type        = string
}

variable "namespace" {
  description = "Kubernetes namespace for eRPC resources."
  type        = string
  default     = "default"
}

variable "image" {
  description = "Container image for eRPC."
  type        = string
  default     = "ghcr.io/erpc/erpc:latest"
}

variable "container_port" {
  description = "Port eRPC listens on."
  type        = number
  default     = 4000
}

variable "erpc_config_yaml" {
  description = "Full erpc.yaml content, injected into a ConfigMap."
  type        = string
}

variable "cpu_request" {
  description = "CPU request for the eRPC container."
  type        = string
  default     = "256m"
}

variable "memory_request" {
  description = "Memory request for the eRPC container."
  type        = string
  default     = "512Mi"
}

variable "cpu_limit" {
  description = "CPU limit for the eRPC container."
  type        = string
  default     = "500m"
}

variable "memory_limit" {
  description = "Memory limit for the eRPC container."
  type        = string
  default     = "1Gi"
}

variable "monitoring_enabled" {
  description = "Whether monitoring stack is enabled (controls ServiceMonitor creation)."
  type        = bool
  default     = false
}

variable "wait_for_rollout" {
  description = "Wait for the Deployment rollout to complete."
  type        = bool
  default     = true
}
