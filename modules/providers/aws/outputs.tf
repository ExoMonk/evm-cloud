output "adapter_context" {
  description = "Resolved AWS adapter context from provider-neutral root inputs."
  value       = local.adapter_context
}

output "networking" {
  description = "Networking outputs from AWS adapter, or null when disabled."
  value       = local.networking
}
