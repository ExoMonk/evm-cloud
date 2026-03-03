variable "enabled" {
  description = "Enable External Secrets Operator deployment."
  type        = bool
  default     = false
}

variable "eso_chart_version" {
  description = "External Secrets Operator Helm chart version. Pinned for reproducibility."
  type        = string
  default     = "0.9.13"
}
