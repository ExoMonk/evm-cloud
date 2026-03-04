resource "kubernetes_manifest" "erpc_service_monitor" {
  count = var.monitoring_enabled ? 1 : 0

  manifest = {
    apiVersion = "monitoring.coreos.com/v1"
    kind       = "ServiceMonitor"
    metadata = {
      name      = "${var.project_name}-erpc"
      namespace = var.namespace
      labels = {
        app = "${var.project_name}-erpc"
      }
    }
    spec = {
      selector = {
        matchLabels = {
          app = "${var.project_name}-erpc"
        }
      }
      endpoints = [
        {
          port     = "metrics"
          path     = "/metrics"
          interval = "15s"
        }
      ]
    }
  }
}
