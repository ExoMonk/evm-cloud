# Grafana dashboard ConfigMaps — auto-loaded by Grafana sidecar via grafana_dashboard label.
# These are the EKS (Terraform) equivalent of deployers/charts/dashboards/templates/*.yaml.

resource "kubernetes_config_map" "rindexer_dashboard" {
  count      = var.enabled ? 1 : 0
  depends_on = [helm_release.kube_prometheus_stack]

  metadata {
    name      = "${var.project_name}-rindexer-dashboard"
    namespace = var.namespace
    labels = {
      grafana_dashboard = "1"
    }
  }

  data = {
    "rindexer.json" = jsonencode({
      annotations          = { list = [] }
      editable             = true
      fiscalYearStartMonth = 0
      graphTooltip         = 1
      links                = []
      schemaVersion        = 39
      tags                 = ["rindexer", "indexer"]
      templating           = { list = [] }
      time                 = { from = "now-1h", to = "now" }
      title                = "rindexer"
      uid                  = "rindexer-overview"
      panels = [
        {
          title   = "Blocks Behind (by network)"
          type    = "timeseries"
          gridPos = { h = 8, w = 12, x = 0, y = 0 }
          targets = [{ expr = "max by (network) (rindexer_blocks_behind)", legendFormat = "{{network}}" }]
          fieldConfig = {
            defaults = {
              custom = { drawStyle = "line", fillOpacity = 10 }
              unit   = "short"
              thresholds = { mode = "absolute", steps = [
                { color = "green", value = null },
                { color = "yellow", value = 100 },
                { color = "red", value = 1000 }
              ] }
            }
          }
        },
        {
          title       = "Events Processed / sec"
          type        = "timeseries"
          gridPos     = { h = 8, w = 12, x = 12, y = 0 }
          targets     = [{ expr = "sum by (network, contract) (rate(rindexer_events_processed_total[5m]))", legendFormat = "{{network}}/{{contract}}" }]
          fieldConfig = { defaults = { custom = { drawStyle = "line", fillOpacity = 10 }, unit = "ops" } }
        },
        {
          title       = "RPC Error Rate"
          type        = "timeseries"
          gridPos     = { h = 8, w = 12, x = 0, y = 8 }
          targets     = [{ expr = "sum by (network) (rate(rindexer_rpc_requests_total{status=\"error\"}[5m])) / sum by (network) (rate(rindexer_rpc_requests_total[5m]))", legendFormat = "{{network}}" }]
          fieldConfig = { defaults = { custom = { drawStyle = "line", fillOpacity = 10 }, unit = "percentunit", max = 1 } }
        },
        {
          title       = "DB Operations / sec"
          type        = "timeseries"
          gridPos     = { h = 8, w = 12, x = 12, y = 8 }
          targets     = [{ expr = "sum by (operation, status) (rate(rindexer_db_operations_total[5m]))", legendFormat = "{{operation}} ({{status}})" }]
          fieldConfig = { defaults = { custom = { drawStyle = "line", fillOpacity = 10 }, unit = "ops" } }
        },
        {
          title       = "Reorgs Detected"
          type        = "timeseries"
          gridPos     = { h = 8, w = 12, x = 0, y = 16 }
          targets     = [{ expr = "increase(rindexer_reorgs_detected_total[15m])", legendFormat = "{{network}}" }]
          fieldConfig = { defaults = { custom = { drawStyle = "bars", fillOpacity = 50 }, unit = "short" } }
        },
        {
          title   = "Reorg Depth"
          type    = "stat"
          gridPos = { h = 8, w = 12, x = 12, y = 16 }
          targets = [{ expr = "max by (network) (rindexer_reorg_depth)", legendFormat = "{{network}}" }]
          fieldConfig = {
            defaults = {
              unit = "short"
              thresholds = { mode = "absolute", steps = [
                { color = "green", value = null },
                { color = "yellow", value = 2 },
                { color = "red", value = 5 }
              ] }
            }
          }
        }
      ]
    })
  }
}

resource "kubernetes_config_map" "erpc_dashboard" {
  count      = var.enabled ? 1 : 0
  depends_on = [helm_release.kube_prometheus_stack]

  metadata {
    name      = "${var.project_name}-erpc-dashboard"
    namespace = var.namespace
    labels = {
      grafana_dashboard = "1"
    }
  }

  data = {
    "erpc.json" = jsonencode({
      annotations          = { list = [] }
      editable             = true
      fiscalYearStartMonth = 0
      graphTooltip         = 1
      links                = []
      schemaVersion        = 39
      tags                 = ["erpc", "rpc-proxy"]
      templating           = { list = [] }
      time                 = { from = "now-1h", to = "now" }
      title                = "eRPC"
      uid                  = "erpc-overview"
      panels = [
        {
          title       = "Request Rate"
          type        = "timeseries"
          gridPos     = { h = 8, w = 12, x = 0, y = 0 }
          targets     = [{ expr = "sum by (network) (rate(erpc_requests_total[5m]))", legendFormat = "{{network}}" }]
          fieldConfig = { defaults = { custom = { drawStyle = "line", fillOpacity = 10 }, unit = "reqps" } }
        },
        {
          title       = "Error Rate"
          type        = "timeseries"
          gridPos     = { h = 8, w = 12, x = 12, y = 0 }
          targets     = [{ expr = "sum by (network) (rate(erpc_errors_total[5m])) / sum by (network) (rate(erpc_requests_total[5m]))", legendFormat = "{{network}}" }]
          fieldConfig = { defaults = { custom = { drawStyle = "line", fillOpacity = 10 }, unit = "percentunit", max = 1 } }
        },
        {
          title       = "Upstream Latency (p99)"
          type        = "timeseries"
          gridPos     = { h = 8, w = 12, x = 0, y = 8 }
          targets     = [{ expr = "histogram_quantile(0.99, sum by (le, upstream) (rate(erpc_upstream_request_duration_seconds_bucket[5m])))", legendFormat = "{{upstream}}" }]
          fieldConfig = { defaults = { custom = { drawStyle = "line", fillOpacity = 10 }, unit = "s" } }
        },
        {
          title   = "Cache Hit Rate"
          type    = "stat"
          gridPos = { h = 8, w = 12, x = 12, y = 8 }
          targets = [{ expr = "sum(rate(erpc_cache_hits_total[5m])) / (sum(rate(erpc_cache_hits_total[5m])) + sum(rate(erpc_cache_misses_total[5m])))", legendFormat = "hit rate" }]
          fieldConfig = {
            defaults = {
              unit = "percentunit"
              thresholds = { mode = "absolute", steps = [
                { color = "red", value = null },
                { color = "yellow", value = 0.5 },
                { color = "green", value = 0.8 }
              ] }
            }
          }
        },
        {
          title       = "Upstream Health"
          type        = "timeseries"
          gridPos     = { h = 8, w = 24, x = 0, y = 16 }
          targets     = [{ expr = "erpc_upstream_healthy", legendFormat = "{{upstream}}" }]
          fieldConfig = { defaults = { custom = { drawStyle = "line", fillOpacity = 30 }, unit = "bool", min = 0, max = 1 } }
        }
      ]
    })
  }
}

resource "kubernetes_config_map" "infra_dashboard" {
  count      = var.enabled ? 1 : 0
  depends_on = [helm_release.kube_prometheus_stack]

  metadata {
    name      = "${var.project_name}-infra-dashboard"
    namespace = var.namespace
    labels = {
      grafana_dashboard = "1"
    }
  }

  data = {
    "infra.json" = jsonencode({
      annotations          = { list = [] }
      editable             = true
      fiscalYearStartMonth = 0
      graphTooltip         = 1
      links                = []
      schemaVersion        = 39
      tags                 = ["infra", "kubernetes"]
      templating           = { list = [] }
      time                 = { from = "now-1h", to = "now" }
      title                = "Infrastructure"
      uid                  = "infra-overview"
      panels = [
        {
          title       = "CPU Usage by Pod"
          type        = "timeseries"
          gridPos     = { h = 8, w = 12, x = 0, y = 0 }
          targets     = [{ expr = "sum by (pod) (rate(container_cpu_usage_seconds_total{namespace=\"default\", container!=\"\", container!=\"POD\"}[5m]))", legendFormat = "{{pod}}" }]
          fieldConfig = { defaults = { custom = { drawStyle = "line", fillOpacity = 10 }, unit = "short" } }
        },
        {
          title       = "Memory Usage by Pod"
          type        = "timeseries"
          gridPos     = { h = 8, w = 12, x = 12, y = 0 }
          targets     = [{ expr = "sum by (pod) (container_memory_working_set_bytes{namespace=\"default\", container!=\"\", container!=\"POD\"})", legendFormat = "{{pod}}" }]
          fieldConfig = { defaults = { custom = { drawStyle = "line", fillOpacity = 10 }, unit = "bytes" } }
        },
        {
          title   = "Network I/O by Pod"
          type    = "timeseries"
          gridPos = { h = 8, w = 12, x = 0, y = 8 }
          targets = [
            { expr = "sum by (pod) (rate(container_network_receive_bytes_total{namespace=\"default\"}[5m]))", legendFormat = "{{pod}} rx" },
            { expr = "sum by (pod) (rate(container_network_transmit_bytes_total{namespace=\"default\"}[5m]))", legendFormat = "{{pod}} tx" }
          ]
          fieldConfig = { defaults = { custom = { drawStyle = "line", fillOpacity = 10 }, unit = "Bps" } }
        },
        {
          title   = "Disk Usage"
          type    = "timeseries"
          gridPos = { h = 8, w = 12, x = 12, y = 8 }
          targets = [{ expr = "1 - node_filesystem_avail_bytes{mountpoint=\"/\"} / node_filesystem_size_bytes{mountpoint=\"/\"}", legendFormat = "{{instance}}" }]
          fieldConfig = {
            defaults = {
              custom = { drawStyle = "line", fillOpacity = 10 }
              unit   = "percentunit"
              max    = 1
              thresholds = { mode = "absolute", steps = [
                { color = "green", value = null },
                { color = "yellow", value = 0.8 },
                { color = "red", value = 0.9 }
              ] }
            }
          }
        },
        {
          title   = "Pod Restart Count"
          type    = "stat"
          gridPos = { h = 8, w = 12, x = 0, y = 16 }
          targets = [{ expr = "sum by (pod) (increase(kube_pod_container_status_restarts_total{namespace=\"default\"}[1h]))", legendFormat = "{{pod}}" }]
          fieldConfig = {
            defaults = {
              unit = "short"
              thresholds = { mode = "absolute", steps = [
                { color = "green", value = null },
                { color = "yellow", value = 3 },
                { color = "red", value = 10 }
              ] }
            }
          }
        },
        {
          title   = "Node CPU Usage"
          type    = "gauge"
          gridPos = { h = 8, w = 12, x = 12, y = 16 }
          targets = [{ expr = "1 - avg by (instance) (irate(node_cpu_seconds_total{mode=\"idle\"}[5m]))", legendFormat = "{{instance}}" }]
          fieldConfig = {
            defaults = {
              unit = "percentunit"
              max  = 1
              thresholds = { mode = "absolute", steps = [
                { color = "green", value = null },
                { color = "yellow", value = 0.7 },
                { color = "red", value = 0.9 }
              ] }
            }
          }
        }
      ]
    })
  }
}
