resource "kubernetes_manifest" "indexer_service_monitor" {
  count = var.monitoring_enabled ? 1 : 0

  manifest = {
    apiVersion = "monitoring.coreos.com/v1"
    kind       = "ServiceMonitor"
    metadata = {
      name      = "${var.project_name}-indexer"
      namespace = var.namespace
      labels = {
        app = "${var.project_name}-indexer"
      }
    }
    spec = {
      selector = {
        matchLabels = {
          app = "${var.project_name}-indexer"
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
