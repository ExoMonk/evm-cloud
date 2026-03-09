locals {
  is_postgres = var.storage_backend == "postgres"

  # Stable secret name — used by both inline (kubernetes_secret) and ESO (ExternalSecret target).
  secret_name = "${var.project_name}-indexer-secrets"

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
    },
    var.extra_env,
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

# Secrets (DATABASE_URL or CLICKHOUSE_PASSWORD) — inline mode only.
# In provider/external mode, ESO creates a K8s Secret with the same name via ExternalSecret.
resource "kubernetes_secret" "indexer" {
  #checkov:skip=CKV_K8S_21:Default namespace acceptable for Tier 0
  count = var.secrets_mode == "inline" ? 1 : 0

  metadata {
    name      = local.secret_name
    namespace = var.namespace
  }

  data = local.secret_data
}

# ExternalSecret for provider/external mode — creates a K8s Secret synced from a backing store.
# Uses null_resource + kubectl apply to avoid kubernetes_manifest CRD-at-plan-time issues.
# Shape must match deployers/charts/indexer/templates/external-secret.yaml exactly.
resource "null_resource" "external_secret" {
  count = var.secrets_mode != "inline" ? 1 : 0

  triggers = {
    secret_name  = local.secret_name
    namespace    = var.namespace
    store_name   = var.secrets_store_name
    store_kind   = var.secrets_store_kind
    secret_key   = var.secrets_secret_key
    cluster_name = var.eks_cluster_name
    region       = var.aws_region
  }

  provisioner "local-exec" {
    command = <<-EOT
      set -e
      KUBECONFIG_PATH="/tmp/evm-cloud-eks-${self.triggers.cluster_name}-$$.kubeconfig"
      trap 'rm -f "$KUBECONFIG_PATH"' EXIT
      aws eks update-kubeconfig --name ${self.triggers.cluster_name} \
        --region ${self.triggers.region} \
        --kubeconfig "$KUBECONFIG_PATH"
      export KUBECONFIG="$KUBECONFIG_PATH"
      cat <<EOF | kubectl apply -f -
apiVersion: external-secrets.io/v1beta1
kind: ExternalSecret
metadata:
  name: ${self.triggers.secret_name}
  namespace: ${self.triggers.namespace}
spec:
  refreshInterval: 60s
  secretStoreRef:
    name: ${self.triggers.store_name}
    kind: ${self.triggers.store_kind}
  target:
    name: ${self.triggers.secret_name}
  dataFrom:
    - extract:
        key: ${self.triggers.secret_key}
EOF
    EOT
  }

  provisioner "local-exec" {
    when       = destroy
    on_failure = continue
    command    = <<-EOT
      KUBECONFIG_PATH="/tmp/evm-cloud-eks-${self.triggers.cluster_name}-$$.kubeconfig"
      trap 'rm -f "$KUBECONFIG_PATH"' EXIT
      aws eks update-kubeconfig --name ${self.triggers.cluster_name} \
        --region ${self.triggers.region} \
        --kubeconfig "$KUBECONFIG_PATH"
      KUBECONFIG="$KUBECONFIG_PATH" \
        kubectl delete externalsecret ${self.triggers.secret_name} \
        -n ${self.triggers.namespace} --ignore-not-found
    EOT
  }
}

# Single-writer constraint: rindexer must run exactly one active writer per
# dataset. replicas=1 with Recreate strategy prevents two pods running during rollout.
resource "kubernetes_service" "indexer" {
  #checkov:skip=CKV_K8S_21:Default namespace acceptable for Tier 0
  metadata {
    name      = "${var.project_name}-indexer"
    namespace = var.namespace
  }

  spec {
    selector = {
      app = "${var.project_name}-indexer"
    }

    port {
      name        = "metrics"
      port        = 8080
      target_port = 8080
    }

    type = "ClusterIP"
  }
}

resource "kubernetes_deployment" "indexer" {
  #checkov:skip=CKV_K8S_14:Image tag pinning is user responsibility via var.image
  #checkov:skip=CKV_K8S_21:Default namespace acceptable for Tier 0
  #checkov:skip=CKV_K8S_28:NET_RAW drop deferred to Tier 1
  #checkov:skip=CKV_K8S_29:Pod security context deferred to Tier 1
  #checkov:skip=CKV_K8S_30:Container security context deferred to Tier 1
  #checkov:skip=CKV_K8S_43:Image digest pinning is user responsibility via var.image
  wait_for_rollout = var.wait_for_rollout

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
          name              = "indexer"
          image             = var.image
          image_pull_policy = "Always"
          args              = ["start", "--path", "/config", "all"]

          port {
            name           = "metrics"
            container_port = 8080
            protocol       = "TCP"
          }

          startup_probe {
            http_get {
              path = "/health"
              port = "metrics"
            }
            failure_threshold = 30
            period_seconds    = 10
          }

          readiness_probe {
            http_get {
              path = "/health"
              port = "metrics"
            }
            initial_delay_seconds = 5
            period_seconds        = 10
          }

          liveness_probe {
            tcp_socket {
              port = 8080
            }
            initial_delay_seconds = 10
            period_seconds        = 20
            failure_threshold     = 5
          }

          # Plain env vars
          dynamic "env" {
            for_each = local.plain_env
            content {
              name  = env.key
              value = env.value
            }
          }

          # Secret env vars — references the K8s Secret by stable name.
          # In inline mode, Terraform creates the secret; in provider/external, ESO syncs it.
          dynamic "env" {
            for_each = local.secret_data
            content {
              name = env.key
              value_from {
                secret_key_ref {
                  name = local.secret_name
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
