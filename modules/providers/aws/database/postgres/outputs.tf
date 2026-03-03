output "endpoint" {
  description = "RDS instance endpoint address."
  value       = module.rds.db_instance_address
}

output "port" {
  description = "RDS instance port."
  value       = module.rds.db_instance_port
}

output "db_name" {
  description = "Name of the database."
  value       = module.rds.db_instance_name
}

output "master_username" {
  description = "Master username for the database."
  value       = var.db_username
}

output "master_password" {
  description = "Master password for the database. Only available when master_password is provided; null when AWS-managed."
  value       = var.master_password
  sensitive   = true
}

output "master_secret_arn" {
  description = "ARN of the Secrets Manager secret containing master credentials. Null when master_password is provided (user manages their own secret)."
  value       = local.use_explicit_password ? null : module.rds.db_instance_master_user_secret_arn
}
