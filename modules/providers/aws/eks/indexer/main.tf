locals {
  is_postgres = var.storage_backend == "postgres"

  secret_data = local.is_postgres ? {
    DATABASE_URL = var.database_url
    } : {
    CLICKHOUSE_PASSWORD = var.clickhouse_password
  }

  # Non-sensitive env vars
  plain_env = merge(
    { RPC_URL = var.rpc_url },
    local.is_postgres ? {} : {
      CLICKHOUSE_URL  = var.clickhouse_url
      CLICKHOUSE_USER = var.clickhouse_user
      CLICKHOUSE_DB   = var.clickhouse_db
    }
  )
}

# rindexer.yaml config
resource "kubernetes_config_map" "config" {
  #checkov:skip=CKV_K8S_21:Default namespace acceptable for Tier 0
  metadata {
    name      = "${var.project_name}-indexer-config"
    namespace = var.namespace
  }

  data = {
    "rindexer.yaml" = var.rindexer_config_yaml
  }
}

# ABI files (separate ConfigMap for clean volume mount)
resource "kubernetes_config_map" "abis" {
  #checkov:skip=CKV_K8S_21:Default namespace acceptable for Tier 0
  metadata {
    name      = "${var.project_name}-indexer-abis"
    namespace = var.namespace
  }

  data = var.rindexer_abis
}

# Secrets (DATABASE_URL or CLICKHOUSE_PASSWORD)
resource "kubernetes_secret" "indexer" {
  #checkov:skip=CKV_K8S_21:Default namespace acceptable for Tier 0
  metadata {
    name      = "${var.project_name}-indexer-secrets"
    namespace = var.namespace
  }

  data = local.secret_data
}

# Single-writer constraint: rindexer must run exactly one active writer per
# dataset. replicas=1 with Recreate strategy prevents two pods running during rollout.
resource "kubernetes_deployment" "indexer" {
  #checkov:skip=CKV_K8S_8:Liveness probe deferred to Tier 1
  #checkov:skip=CKV_K8S_9:Readiness probe deferred to Tier 1
  #checkov:skip=CKV_K8S_14:Image tag pinning is user responsibility via var.image
  #checkov:skip=CKV_K8S_21:Default namespace acceptable for Tier 0
  #checkov:skip=CKV_K8S_28:NET_RAW drop deferred to Tier 1
  #checkov:skip=CKV_K8S_29:Pod security context deferred to Tier 1
  #checkov:skip=CKV_K8S_30:Container security context deferred to Tier 1
  #checkov:skip=CKV_K8S_43:Image digest pinning is user responsibility via var.image
  metadata {
    name      = "${var.project_name}-indexer"
    namespace = var.namespace
    labels = {
      app = "${var.project_name}-indexer"
    }
  }

  spec {
    replicas = 1

    strategy {
      type = "Recreate"
    }

    selector {
      match_labels = {
        app = "${var.project_name}-indexer"
      }
    }

    template {
      metadata {
        labels = {
          app = "${var.project_name}-indexer"
        }
      }

      spec {
        container {
          name  = "indexer"
          image = var.image
          args  = ["start", "--path", "/config"]

          # Plain env vars
          dynamic "env" {
            for_each = local.plain_env
            content {
              name  = env.key
              value = env.value
            }
          }

          # Secret env vars
          dynamic "env" {
            for_each = local.secret_data
            content {
              name = env.key
              value_from {
                secret_key_ref {
                  name = kubernetes_secret.indexer.metadata[0].name
                  key  = env.key
                }
              }
            }
          }

          volume_mount {
            name       = "config"
            mount_path = "/config"
            read_only  = true
          }

          volume_mount {
            name       = "abis"
            mount_path = "/config/abis"
            read_only  = true
          }

          resources {
            requests = {
              cpu    = var.cpu_request
              memory = var.memory_request
            }
            limits = {
              cpu    = var.cpu_limit
              memory = var.memory_limit
            }
          }
        }

        volume {
          name = "config"
          config_map {
            name = kubernetes_config_map.config.metadata[0].name
          }
        }

        volume {
          name = "abis"
          config_map {
            name = kubernetes_config_map.abis.metadata[0].name
          }
        }
      }
    }
  }
}
