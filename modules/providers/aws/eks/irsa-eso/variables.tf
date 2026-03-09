variable "oidc_provider_arn" {
  description = "ARN of the EKS OIDC provider for IRSA federation."
  type        = string
}

variable "project_name" {
  description = "Project name, used for IAM resource naming."
  type        = string
}

variable "secret_arns" {
  description = "List of Secrets Manager ARNs the ESO role can read."
  type        = list(string)
}

variable "kms_key_arn" {
  description = "KMS key ARN for decrypting secrets. Empty string skips KMS permissions."
  type        = string
  default     = ""
}

variable "eso_namespace" {
  description = "Kubernetes namespace where ESO is installed."
  type        = string
  default     = "external-secrets"
}

variable "eso_service_account_name" {
  description = "Name of the ESO ServiceAccount for IRSA trust policy."
  type        = string
  default     = "external-secrets"
}
