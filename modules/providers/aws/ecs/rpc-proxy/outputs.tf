output "service_name" {
  description = "ECS service name for the eRPC proxy."
  value       = module.ecs_service.name
}

output "container_port" {
  description = "Port the eRPC proxy listens on."
  value       = var.container_port
}

output "task_execution_role_arn" {
  description = "IAM task execution role ARN used by the eRPC ECS service."
  value       = module.ecs_service.task_exec_iam_role_arn
}

output "task_role_arn" {
  description = "IAM task role ARN used by the eRPC ECS service."
  value       = module.ecs_service.tasks_iam_role_arn
}
