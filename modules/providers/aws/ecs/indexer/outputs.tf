output "service_name" {
  description = "ECS service name for the rindexer indexer."
  value       = module.ecs_service.name
}

output "log_group_name" {
  description = "CloudWatch log group name for the indexer."
  value       = aws_cloudwatch_log_group.this.name
}

output "task_execution_role_arn" {
  description = "IAM task execution role ARN used by the indexer ECS service."
  value       = module.ecs_service.task_exec_iam_role_arn
}

output "task_role_arn" {
  description = "IAM task role ARN used by the indexer ECS service."
  value       = module.ecs_service.tasks_iam_role_arn
}
