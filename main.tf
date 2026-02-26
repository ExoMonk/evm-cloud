locals {
  supported_providers = ["aws"]
}

module "capabilities" {
  source = "./modules/core/capabilities"

  infrastructure_provider = var.infrastructure_provider
  deployment_target       = var.deployment_target
  runtime_arch            = var.runtime_arch
  database_mode           = var.database_mode
  streaming_mode          = var.streaming_mode
  ingress_mode            = var.ingress_mode
}

resource "terraform_data" "provider_guardrails" {
  input = {
    provider = var.infrastructure_provider
  }

  lifecycle {
    precondition {
      condition     = contains(local.supported_providers, var.infrastructure_provider)
      error_message = "Unsupported infrastructure_provider. Implemented adapters: aws. Add modules/providers/<provider> before using a different value."
    }

    precondition {
      condition     = !(var.infrastructure_provider != "aws" && var.ingress_mode == "managed_lb")
      error_message = "ingress_mode=managed_lb currently requires infrastructure_provider=aws."
    }

    precondition {
      condition     = !(var.infrastructure_provider != "aws" && var.database_mode == "managed")
      error_message = "database_mode=managed currently requires infrastructure_provider=aws."
    }

    precondition {
      condition     = !(var.infrastructure_provider != "aws" && var.streaming_mode == "managed")
      error_message = "streaming_mode=managed currently requires infrastructure_provider=aws."
    }
  }
}

module "provider_aws" {
  source = "./modules/providers/aws"
  count  = var.infrastructure_provider == "aws" ? 1 : 0

  project_name      = var.project_name
  deployment_target = var.deployment_target
  runtime_arch      = var.runtime_arch
  database_mode     = var.database_mode
  streaming_mode    = var.streaming_mode
  ingress_mode      = var.ingress_mode
}
