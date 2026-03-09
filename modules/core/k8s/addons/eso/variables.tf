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

variable "service_account_role_arn" {
  description = "IRSA role ARN to annotate the ESO ServiceAccount. Empty string skips annotation."
  type        = string
  default     = ""
}
