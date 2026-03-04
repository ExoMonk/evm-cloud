# Monitoring addon — kube-prometheus-stack + optional Loki + Promtail.
#
# Single Prometheus instance scrapes ALL ServiceMonitors cluster-wide.
# Single Grafana instance serves all dashboards from ConfigMap sidecar.
# Alertmanager routes rindexer/eRPC alerts to Slack, SNS, or PagerDuty.

locals {
  release_name = "${var.project_name}-monitoring"

  # Alertmanager secret volumes — mount referenced secrets for file-based credential access
  slack_secret_volume = var.alertmanager_route_target == "slack" && var.alertmanager_slack_webhook_secret_name != "" ? [{
    name = "slack-secret"
    secret = {
      secretName = var.alertmanager_slack_webhook_secret_name
    }
  }] : []

  pagerduty_secret_volume = var.alertmanager_route_target == "pagerduty" && var.alertmanager_pagerduty_routing_key_secret_name != "" ? [{
    name = "pagerduty-secret"
    secret = {
      secretName = var.alertmanager_pagerduty_routing_key_secret_name
    }
  }] : []

  alertmanager_secret_volumes = concat(local.slack_secret_volume, local.pagerduty_secret_volume)

  # Build alertmanager config based on route target
  alertmanager_config = {
    alertmanager = {
      config = {
        global = { resolve_timeout = "5m" }
        route = {
          receiver        = "null"
          group_by        = ["alertname", "namespace"]
          group_wait      = "30s"
          group_interval  = "5m"
          repeat_interval = "4h"
          routes = [{
            receiver = var.alertmanager_route_target
            matchers = ["alertname=~\"Rindexer.*|eRPC.*\""]
          }]
        }
        inhibit_rules = [{
          source_matchers = ["severity=critical", "alertname=RindexerCriticalLag"]
          target_matchers = ["severity=warning", "alertname=RindexerHighLag"]
          equal           = ["network"]
        }]
        receivers = [
          { name = "null" },
          {
            name = "slack"
            slack_configs = var.alertmanager_route_target == "slack" ? [{
              api_url_file  = "/etc/alertmanager/secrets/slack/webhook_url"
              channel       = var.alertmanager_slack_channel
              send_resolved = true
              title         = "[{{ .Status | toUpper }}] {{ .CommonLabels.alertname }}"
              text          = "{{ range .Alerts }}*{{ .Labels.alertname }}*\n{{ .Annotations.description }}\n{{ end }}"
            }] : []
          },
          {
            name = "sns"
            sns_configs = var.alertmanager_route_target == "sns" ? [{
              sigv4 = {
                region = var.aws_region
              }
              topic_arn = var.alertmanager_sns_topic_arn
            }] : []
          },
          {
            name = "pagerduty"
            pagerduty_configs = var.alertmanager_route_target == "pagerduty" ? [{
              routing_key_file = "/etc/alertmanager/secrets/pagerduty/routing_key"
            }] : []
          }
        ]
      }
    }
  }
}

resource "helm_release" "kube_prometheus_stack" {
  count = var.enabled ? 1 : 0

  name             = local.release_name
  repository       = "https://prometheus-community.github.io/helm-charts"
  chart            = "kube-prometheus-stack"
  version          = var.kube_prometheus_stack_version
  namespace        = var.namespace
  create_namespace = true
  atomic           = true
  timeout          = 600

  # --- Prometheus resource limits ---
  # Lightweight defaults: single rindexer + eRPC = ~few hundred series.
  # Prometheus runs fine with 128Mi for small deployments; limit at 512Mi for headroom.
  set {
    name  = "prometheus.prometheusSpec.resources.requests.memory"
    value = "128Mi"
  }
  set {
    name  = "prometheus.prometheusSpec.resources.limits.memory"
    value = "512Mi"
  }
  set {
    name  = "prometheus.prometheusSpec.resources.requests.cpu"
    value = "100m"
  }

  # --- Grafana resource limits ---
  set {
    name  = "grafana.resources.requests.memory"
    value = "64Mi"
  }
  set {
    name  = "grafana.resources.limits.memory"
    value = "196Mi"
  }
  set {
    name  = "grafana.resources.requests.cpu"
    value = "50m"
  }

  # --- Retention + storage ---
  set {
    name  = "prometheus.prometheusSpec.retention"
    value = "7d"
  }
  set {
    name  = "prometheus.prometheusSpec.retentionSize"
    value = "8GB"
  }
  set {
    name  = "prometheus.prometheusSpec.storageSpec.volumeClaimTemplate.spec.accessModes[0]"
    value = "ReadWriteOnce"
  }
  set {
    name  = "prometheus.prometheusSpec.storageSpec.volumeClaimTemplate.spec.resources.requests.storage"
    value = "10Gi"
  }
  set {
    name  = "prometheus.prometheusSpec.walCompression"
    value = "true"
  }

  # --- ServiceMonitor discovery: scrape from ALL releases, not just this one ---
  set {
    name  = "prometheus.prometheusSpec.serviceMonitorSelectorNilUsesHelmValues"
    value = "false"
  }
  set {
    name  = "prometheus.prometheusSpec.podMonitorSelectorNilUsesHelmValues"
    value = "false"
  }
  set {
    name  = "prometheus.prometheusSpec.ruleSelectorNilUsesHelmValues"
    value = "false"
  }

  # --- Grafana admin credential from existing secret ---
  set {
    name  = "grafana.admin.existingSecret"
    value = var.grafana_admin_password_secret_name
  }
  set {
    name  = "grafana.admin.userKey"
    value = "admin-user"
  }
  set {
    name  = "grafana.admin.passwordKey"
    value = "admin-password"
  }

  # --- Grafana dashboard sidecar ---
  set {
    name  = "grafana.sidecar.dashboards.enabled"
    value = "true"
  }
  set {
    name  = "grafana.sidecar.dashboards.label"
    value = "grafana_dashboard"
  }
  set {
    name  = "grafana.sidecar.dashboards.searchNamespace"
    value = "ALL"
  }

  # --- Grafana Ingress (piggybacks on Spec 16 ingress infrastructure) ---
  dynamic "set" {
    for_each = var.grafana_ingress_enabled ? [1] : []
    content {
      name  = "grafana.ingress.enabled"
      value = "true"
    }
  }
  dynamic "set" {
    for_each = var.grafana_ingress_enabled ? [1] : []
    content {
      name  = "grafana.ingress.hosts[0]"
      value = var.grafana_hostname
    }
  }
  dynamic "set" {
    for_each = var.grafana_ingress_enabled ? [1] : []
    content {
      name  = "grafana.ingress.ingressClassName"
      value = var.ingress_class_name
    }
  }
  dynamic "set" {
    for_each = var.grafana_ingress_enabled && var.grafana_hostname != "" ? [1] : []
    content {
      name  = "grafana.grafana\\.ini.server.root_url"
      value = "https://${var.grafana_hostname}"
    }
  }

  # --- Loki datasource (if Loki enabled) ---
  dynamic "set" {
    for_each = var.loki_enabled ? [1] : []
    content {
      name  = "grafana.additionalDataSources[0].name"
      value = "Loki"
    }
  }
  dynamic "set" {
    for_each = var.loki_enabled ? [1] : []
    content {
      name  = "grafana.additionalDataSources[0].type"
      value = "loki"
    }
  }
  dynamic "set" {
    for_each = var.loki_enabled ? [1] : []
    content {
      name  = "grafana.additionalDataSources[0].url"
      value = "http://${var.project_name}-loki:3100"
    }
  }
  dynamic "set" {
    for_each = var.loki_enabled ? [1] : []
    content {
      name  = "grafana.additionalDataSources[0].access"
      value = "proxy"
    }
  }

  # --- Alertmanager routing config (Slack/SNS/PagerDuty via secret refs) ---
  values = [yamlencode(local.alertmanager_config)]

  # --- Alertmanager secret volume mounts ---
  dynamic "set" {
    for_each = local.alertmanager_secret_volumes
    content {
      name  = "alertmanager.alertmanagerSpec.secrets[${set.key}]"
      value = set.value.secret.secretName
    }
  }

  # --- Additional scrape for BYO ClickHouse ---
  dynamic "set" {
    for_each = var.clickhouse_metrics_url != "" ? [1] : []
    content {
      name  = "prometheus.prometheusSpec.additionalScrapeConfigs[0].job_name"
      value = "clickhouse"
    }
  }
  dynamic "set" {
    for_each = var.clickhouse_metrics_url != "" ? [1] : []
    content {
      name  = "prometheus.prometheusSpec.additionalScrapeConfigs[0].static_configs[0].targets[0]"
      value = var.clickhouse_metrics_url
    }
  }
  dynamic "set" {
    for_each = var.clickhouse_metrics_url != "" ? [1] : []
    content {
      name  = "prometheus.prometheusSpec.additionalScrapeConfigs[0].metrics_path"
      value = "/metrics"
    }
  }
}

# --- Loki — SingleBinary mode (log storage + query engine) ---
resource "helm_release" "loki" {
  count      = var.enabled && var.loki_enabled ? 1 : 0
  depends_on = [helm_release.kube_prometheus_stack]

  name             = "${var.project_name}-loki"
  repository       = "https://grafana.github.io/helm-charts"
  chart            = "loki"
  version          = var.loki_chart_version
  namespace        = var.namespace
  create_namespace = true
  atomic           = true
  timeout          = 300

  set {
    name  = "deploymentMode"
    value = "SingleBinary"
  }
  set {
    name  = "singleBinary.replicas"
    value = "1"
  }
  set {
    name  = "loki.commonConfig.replication_factor"
    value = "1"
  }
  set {
    name  = "loki.storage.type"
    value = "filesystem"
  }
  set {
    name  = "singleBinary.persistence.enabled"
    value = tostring(var.loki_persistence_enabled)
  }
  set {
    name  = "singleBinary.persistence.size"
    value = "10Gi"
  }
  set {
    name  = "gateway.enabled"
    value = "false"
  }
  set {
    name  = "minio.enabled"
    value = "false"
  }
  set {
    name  = "loki.auth_enabled"
    value = "false"
  }
}

# --- Promtail — DaemonSet that ships container logs from every node to Loki ---
resource "helm_release" "promtail" {
  count      = var.enabled && var.loki_enabled ? 1 : 0
  depends_on = [helm_release.loki]

  name             = "${var.project_name}-promtail"
  repository       = "https://grafana.github.io/helm-charts"
  chart            = "promtail"
  version          = var.promtail_chart_version
  namespace        = var.namespace
  create_namespace = true
  atomic           = true
  timeout          = 300

  set {
    name  = "config.clients[0].url"
    value = "http://${var.project_name}-loki:3100/loki/api/v1/push"
  }
}
