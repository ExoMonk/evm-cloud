variable "project_name" {
  description = "Project name used for resource naming."
  type        = string
}

variable "environment" {
  description = "Deployment environment (dev, production, platform)."
  type        = string
}

variable "subnet_ids" {
  description = "Private subnet IDs for the DB subnet group."
  type        = list(string)
}

variable "security_group_id" {
  description = "Security group ID for database access (from networking module)."
  type        = string
}

variable "engine_version" {
  description = "PostgreSQL engine version."
  type        = string
  default     = "16.4"
}

variable "instance_class" {
  description = "RDS instance class."
  type        = string
  default     = "db.t4g.micro"
}

variable "allocated_storage" {
  description = "Initial allocated storage in GB."
  type        = number
  default     = 20
}

variable "max_allocated_storage" {
  description = "Maximum storage autoscaling limit in GB."
  type        = number
  default     = 100
}

variable "db_name" {
  description = "Name of the database to create."
  type        = string
  default     = "rindexer"
}

variable "db_username" {
  description = "Master username for the database."
  type        = string
  default     = "rindexer"
}

variable "backup_retention_period" {
  description = "Number of days to retain automated backups."
  type        = number
  default     = 7
}

variable "deletion_protection" {
  description = "Enable deletion protection. Defaults to null (auto: true in production, false otherwise)."
  type        = bool
  default     = null
}

variable "multi_az" {
  description = "Enable Multi-AZ deployment. Disabled for Tier 0 single-instance."
  type        = bool
  default     = false
}

variable "manage_master_user_password" {
  description = "Let AWS manage the master password via Secrets Manager (automatic rotation). Set to false and provide master_password for explicit control."
  type        = bool
  default     = true
}

variable "master_password" {
  description = "Explicit master password for RDS. Required when manage_master_user_password = false."
  type        = string
  default     = null
  sensitive   = true
}
