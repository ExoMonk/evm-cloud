resource "kubernetes_config_map" "erpc" {
  #checkov:skip=CKV_K8S_21:Default namespace acceptable for Tier 0
  metadata {
    name      = "${var.project_name}-erpc-config"
    namespace = var.namespace
  }

  data = {
    "erpc.yaml" = var.erpc_config_yaml
  }
}

resource "kubernetes_deployment" "erpc" {
  #checkov:skip=CKV_K8S_8:Liveness probe deferred to Tier 1
  #checkov:skip=CKV_K8S_9:Readiness probe deferred to Tier 1
  #checkov:skip=CKV_K8S_14:Image tag pinning is user responsibility via var.image
  #checkov:skip=CKV_K8S_21:Default namespace acceptable for Tier 0
  #checkov:skip=CKV_K8S_28:NET_RAW drop deferred to Tier 1
  #checkov:skip=CKV_K8S_29:Pod security context deferred to Tier 1
  #checkov:skip=CKV_K8S_30:Container security context deferred to Tier 1
  #checkov:skip=CKV_K8S_43:Image digest pinning is user responsibility via var.image
  wait_for_rollout = var.wait_for_rollout

  metadata {
    name      = "${var.project_name}-erpc"
    namespace = var.namespace
    labels = {
      app = "${var.project_name}-erpc"
    }
  }

  spec {
    replicas = 1

    selector {
      match_labels = {
        app = "${var.project_name}-erpc"
      }
    }

    template {
      metadata {
        labels = {
          app = "${var.project_name}-erpc"
        }
      }

      spec {
        container {
          name  = "erpc"
          image = var.image
          args  = ["--config", "/config/erpc.yaml"]

          port {
            container_port = var.container_port
          }

          volume_mount {
            name       = "config"
            mount_path = "/config"
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
            name = kubernetes_config_map.erpc.metadata[0].name
          }
        }
      }
    }
  }
}

resource "kubernetes_service" "erpc" {
  #checkov:skip=CKV_K8S_21:Default namespace acceptable for Tier 0
  metadata {
    name      = "${var.project_name}-erpc"
    namespace = var.namespace
  }

  spec {
    selector = {
      app = "${var.project_name}-erpc"
    }

    port {
      port        = var.container_port
      target_port = var.container_port
    }

    type = "ClusterIP"
  }
}
