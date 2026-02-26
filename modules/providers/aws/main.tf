locals {
  adapter_context = {
    provider          = "aws"
    project_name      = var.project_name
    deployment_target = var.deployment_target
    runtime_arch      = var.runtime_arch
    database_mode     = var.database_mode
    streaming_mode    = var.streaming_mode
    ingress_mode      = var.ingress_mode
  }
}
