terraform {
  required_version = ">= 1.14.6"

  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.0"
    }
  }
}

provider "kubernetes" {
  config_path    = var.kubeconfig_path
  config_context = var.kubeconfig_context
}

module "rpc_proxy" {
  source = "../../modules/providers/aws/eks/rpc-proxy"

  project_name     = "kind-test"
  erpc_config_yaml = var.erpc_config_yaml
  wait_for_rollout = false
}

module "indexer" {
  source = "../../modules/providers/aws/eks/indexer"

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
