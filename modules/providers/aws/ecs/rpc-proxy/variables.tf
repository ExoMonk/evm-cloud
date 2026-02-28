variable "project_name" {
  description = "Project name used for resource naming."
  type        = string
}

variable "environment" {
  description = "Deployment environment (dev, production, platform)."
  type        = string
}

variable "subnet_ids" {
  description = "Private subnet IDs for ECS service networking."
  type        = list(string)
}

variable "security_group_id" {
  description = "Security group ID for eRPC access (from networking module)."
  type        = string
}

variable "cluster_arn" {
  description = "ARN of the shared ECS cluster."
  type        = string
}

variable "image" {
  description = "Container image for eRPC. Override for multi-arch compatibility."
  type        = string
  default     = "ghcr.io/erpc/erpc:latest"
}

variable "cpu" {
  description = "CPU units for the eRPC task (1024 = 1 vCPU)."
  type        = number
  default     = 512
}

variable "memory" {
  description = "Memory in MiB for the eRPC task."
  type        = number
  default     = 1024
}

variable "container_port" {
  description = "Port eRPC listens on."
  type        = number
  default     = 4000
}

variable "config_bucket_name" {
  description = "S3 bucket name containing the erpc.yaml config file."
  type        = string
}

variable "config_object_key" {
  description = "S3 object key for the erpc.yaml config file."
  type        = string
}

variable "aws_region" {
  description = "AWS region for CloudWatch log group and S3 access."
  type        = string
}
