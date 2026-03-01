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
      compute_engine    = var.compute_engine
      workload_mode     = var.workload_mode
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
