# K8s Addons — Third-party Helm charts managed by Terraform.
#
# Each addon is a sub-module in its own directory (e.g., ./monitoring/, ./ingress/).
# Sub-modules are called conditionally based on enable flags.
# DO NOT add helm_release resources directly in this file — always use sub-modules.
#
# This module inherits both `kubernetes` and `helm` providers from its caller.
# It must be called with explicit `providers = { kubernetes = kubernetes, helm = helm }`.

module "eso" {
  source = "./eso"

  enabled                  = var.eso_enabled
  eso_chart_version        = var.eso_chart_version
  service_account_role_arn = var.eso_service_account_role_arn
}

module "monitoring" {
  source = "./monitoring"

  enabled                                        = var.monitoring_enabled
  project_name                                   = var.project_name
  kube_prometheus_stack_version                  = var.kube_prometheus_stack_version
  grafana_admin_password_secret_name             = var.grafana_admin_password_secret_name
  alertmanager_slack_webhook_secret_name         = var.alertmanager_slack_webhook_secret_name
  alertmanager_sns_topic_arn                     = var.alertmanager_sns_topic_arn
  alertmanager_pagerduty_routing_key_secret_name = var.alertmanager_pagerduty_routing_key_secret_name
  alertmanager_route_target                      = var.alertmanager_route_target
  alertmanager_slack_channel                     = var.alertmanager_slack_channel
  loki_enabled                                   = var.loki_enabled
  loki_chart_version                             = var.loki_chart_version
  promtail_chart_version                         = var.promtail_chart_version
  loki_persistence_enabled                       = var.loki_persistence_enabled
  clickhouse_metrics_url                         = var.clickhouse_metrics_url
  grafana_ingress_enabled                        = var.grafana_ingress_enabled
  grafana_hostname                               = var.grafana_hostname
  ingress_class_name                             = var.ingress_class_name
  aws_region                                     = var.aws_region
}

