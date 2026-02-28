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

output "master_secret_arn" {
  description = "ARN of the Secrets Manager secret containing master credentials."
  value       = module.rds.db_instance_master_user_secret_arn
}
