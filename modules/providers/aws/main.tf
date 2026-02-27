module "networking" {
  source = "../../networking"
  count  = var.networking_enabled ? 1 : 0

  project_name         = var.project_name
  environment          = var.network_environment
  vpc_cidr             = var.network_vpc_cidr
  availability_zones   = var.network_availability_zones
  enable_nat_gateway   = var.network_enable_nat_gateway
  enable_vpc_endpoints = var.network_enable_vpc_endpoints
}

locals {
  networking = var.networking_enabled ? {
    vpc_id             = module.networking[0].vpc_id
    public_subnet_ids  = module.networking[0].public_subnet_ids
    private_subnet_ids = module.networking[0].private_subnet_ids
    security_group_ids = module.networking[0].security_group_ids
  } : null

  adapter_context = {
    provider          = "aws"
    project_name      = var.project_name
    deployment_target = var.deployment_target
    runtime_arch      = var.runtime_arch
    database_mode     = var.database_mode
    streaming_mode    = var.streaming_mode
    ingress_mode      = var.ingress_mode
    aws_region        = var.aws_region
    networking        = local.networking
  }
}
