output "service_name" {
  description = "ECS service name for the rindexer indexer."
  value       = module.ecs_service.name
}

output "log_group_name" {
  description = "CloudWatch log group name for the indexer."
  value       = aws_cloudwatch_log_group.this.name
}
