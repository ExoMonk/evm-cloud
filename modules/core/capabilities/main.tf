locals {
  contract = {
    provider = {
      name = var.infrastructure_provider
    }

    network = {
      required = true
    }

    compute = {
      deployment_target = var.deployment_target
      runtime_arch      = var.runtime_arch
    }

    database = {
      mode = var.database_mode
    }

    ingress = {
      mode = var.ingress_mode
    }

    streaming = {
      mode = var.streaming_mode
    }

    secrets_access = {
      required = true
    }
  }
}
