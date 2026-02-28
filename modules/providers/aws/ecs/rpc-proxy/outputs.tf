output "service_name" {
  description = "ECS service name for the eRPC proxy."
  value       = module.ecs_service.name
}

output "container_port" {
  description = "Port the eRPC proxy listens on."
  value       = var.container_port
}
