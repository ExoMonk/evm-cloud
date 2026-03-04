# PrometheusRule CRD — alert rules + recording rules for rindexer and eRPC.
# Deployed via Terraform kubernetes_manifest for the EKS path.
# The k3s path deploys identical rules via deployers/charts/dashboards/templates/alerts.yaml.

resource "kubernetes_manifest" "rindexer_alerts" {
  count      = var.enabled ? 1 : 0
  depends_on = [helm_release.kube_prometheus_stack]

  manifest = {
    apiVersion = "monitoring.coreos.com/v1"
    kind       = "PrometheusRule"
    metadata = {
      name      = "${var.project_name}-rindexer-alerts"
      namespace = var.namespace
      labels = {
        release = local.release_name
      }
    }
    spec = {
      groups = [
        {
          name = "rindexer.rules"
          rules = [
            {
              alert  = "RindexerHighLag"
              expr   = "max by (network) (rindexer_blocks_behind) > 100"
              for    = "5m"
              labels = { severity = "warning" }
              annotations = {
                summary     = "rindexer lag > 100 blocks on {{ $labels.network }}"
                description = "Max indexer lag on {{ $labels.network }} is {{ $value }} blocks for 5m."
              }
            },
            {
              alert  = "RindexerCriticalLag"
              expr   = "max by (network) (rindexer_blocks_behind) > 1000"
              for    = "10m"
              labels = { severity = "critical" }
              annotations = {
                summary     = "rindexer lag > 1000 blocks on {{ $labels.network }}"
                description = "Max indexer lag on {{ $labels.network }} is {{ $value }} blocks for 10m."
              }
            },
            {
              alert  = "RindexerDeepReorg"
              expr   = "rindexer_reorg_depth > 2"
              for    = "1m"
              labels = { severity = "warning" }
              annotations = {
                summary     = "Deep reorg (depth {{ $value }}) on {{ $labels.network }}"
                description = "Reorg depth > 2 blocks on {{ $labels.network }}. Investigate chain stability."
              }
            },
            {
              alert  = "RindexerFrequentReorgs"
              expr   = "increase(rindexer_reorgs_detected_total[15m]) > 5"
              for    = "5m"
              labels = { severity = "warning" }
              annotations = {
                summary     = "Frequent reorgs on {{ $labels.network }}"
                description = "{{ $value }} reorgs on {{ $labels.network }} in 15m."
              }
            },
            {
              alert  = "RindexerRPCErrorRate"
              expr   = "sum by (network) (rate(rindexer_rpc_requests_total{status=\"error\"}[5m])) / sum by (network) (rate(rindexer_rpc_requests_total[5m])) > 0.05 and sum by (network) (rate(rindexer_rpc_requests_total[5m])) > 0.1"
              for    = "5m"
              labels = { severity = "warning" }
              annotations = {
                summary     = "RPC error rate > 5% on {{ $labels.network }}"
                description = "{{ $labels.network }} RPC error rate is {{ $value | humanizePercentage }}."
              }
            },
            {
              alert  = "RindexerDBErrors"
              expr   = "increase(rindexer_db_operations_total{status=\"error\"}[5m]) > 0"
              for    = "2m"
              labels = { severity = "critical" }
              annotations = {
                summary     = "Database write errors detected"
                description = "{{ $value }} DB errors in the last 5 minutes (operation={{ $labels.operation }})."
              }
            },
            {
              alert  = "RindexerDown"
              expr   = "up{app_kubernetes_io_name=\"indexer\"} == 0"
              for    = "2m"
              labels = { severity = "critical" }
              annotations = {
                summary     = "rindexer is down"
                description = "rindexer target has been down for 2 minutes."
              }
            }
          ]
        },
        {
          name = "rindexer.recording"
          rules = [
            {
              record = "rindexer:blocks_behind:max_by_network"
              expr   = "max by (network) (rindexer_blocks_behind)"
            },
            {
              record = "rindexer:events_per_second:5m"
              expr   = "sum by (network, contract) (rate(rindexer_events_processed_total[5m]))"
            },
            {
              record = "rindexer:rpc_error_rate:5m"
              expr   = "sum by (network) (rate(rindexer_rpc_requests_total{status=\"error\"}[5m])) / sum by (network) (rate(rindexer_rpc_requests_total[5m]))"
            }
          ]
        }
      ]
    }
  }
}
