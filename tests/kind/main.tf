terraform {
  required_version = ">= 1.14.6"

  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.0"
    }
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.12"
    }
  }
}

provider "kubernetes" {
  config_path    = var.kubeconfig_path
  config_context = var.kubeconfig_context
}

provider "helm" {
  kubernetes {
    config_path    = var.kubeconfig_path
    config_context = var.kubeconfig_context
  }
}

module "rpc_proxy" {
  source = "../../modules/core/k8s/rpc-proxy"

  project_name     = "kind-test"
  erpc_config_yaml = var.erpc_config_yaml
  wait_for_rollout = false
}

module "indexer" {
  source = "../../modules/core/k8s/indexer"

  project_name         = "kind-test"
  rpc_url              = "http://localhost:8545"
  rindexer_config_yaml = var.rindexer_config_yaml
  rindexer_abis        = var.rindexer_abis
  storage_backend      = "clickhouse"
  clickhouse_url       = "http://localhost:8123"
  clickhouse_user      = "default"
  clickhouse_password  = "test-password"
  clickhouse_db        = "test_db"
  wait_for_rollout     = false
}

# --- K8s Addons (Helm provider validation) ---

module "addons" {
  source = "../../modules/core/k8s/addons"

  providers = {
    kubernetes = kubernetes
    helm       = helm
  }

  project_name = "kind-test"
}

# Smoke test: deploy a trivial Helm chart to prove the full provider pipeline.
resource "helm_release" "smoke_test" {
  name             = "helm-smoke-test"
  repository       = "https://helm.github.io/examples"
  chart            = "hello-world"
  namespace        = "default"
  create_namespace = false
  wait             = true
  timeout          = 120
}
