variable "enabled" {
  description = "Enable monitoring stack deployment."
  type        = bool
  default     = false
}

variable "project_name" {
  description = "Project name prefix for Helm release naming."
  type        = string
}

variable "namespace" {
  description = "Kubernetes namespace for monitoring resources."
  type        = string
  default     = "monitoring"
}

variable "kube_prometheus_stack_version" {
  description = "kube-prometheus-stack Helm chart version (pinned)."
  type        = string
  default     = "72.6.2"
}

variable "grafana_admin_password_secret_name" {
  description = "Existing Kubernetes Secret name for Grafana admin password. Keys: admin-user, admin-password."
  type        = string
  default     = ""
}

variable "alertmanager_slack_webhook_secret_name" {
  description = "Existing Kubernetes Secret name holding Slack webhook URL (key: webhook_url)."
  type        = string
  default     = ""
}

variable "alertmanager_sns_topic_arn" {
  description = "SNS topic ARN for Alertmanager webhook routing."
  type        = string
  default     = ""
}

variable "alertmanager_pagerduty_routing_key_secret_name" {
  description = "Existing Kubernetes Secret name holding PagerDuty routing key (key: routing_key)."
  type        = string
  default     = ""
}

variable "alertmanager_route_target" {
  description = "Alertmanager receiver target: slack, sns, or pagerduty."
  type        = string
  default     = "slack"

  validation {
    condition     = contains(["slack", "sns", "pagerduty"], var.alertmanager_route_target)
    error_message = "alertmanager_route_target must be one of: slack, sns, pagerduty."
  }
}

variable "alertmanager_slack_channel" {
  description = "Slack channel name for alert delivery."
  type        = string
  default     = "#alerts"
}

variable "loki_enabled" {
  description = "Deploy Loki (SingleBinary) + Promtail for log aggregation."
  type        = bool
  default     = false
}

variable "loki_chart_version" {
  description = "Loki Helm chart version (grafana/loki)."
  type        = string
  default     = "6.24.0"
}

variable "promtail_chart_version" {
  description = "Promtail Helm chart version (grafana/promtail)."
  type        = string
  default     = "6.16.6"
}

variable "loki_persistence_enabled" {
  description = "Enable PVC for Loki. False = logs lost on restart."
  type        = bool
  default     = false
}

variable "clickhouse_metrics_url" {
  description = "Optional BYO ClickHouse metrics endpoint for additional scrape."
  type        = string
  default     = ""
}

variable "grafana_ingress_enabled" {
  description = "Expose Grafana via Ingress. Requires ingress_mode != none."
  type        = bool
  default     = true
}

variable "grafana_hostname" {
  description = "Grafana hostname (e.g., grafana.yourdomain.com). Required when grafana_ingress_enabled = true."
  type        = string
  default     = ""
}

variable "grafana_extra_dashboards" {
  description = "Map of dashboard filename to JSON content for Grafana sidecar loading."
  type        = map(string)
  default     = {}
}

variable "ingress_class_name" {
  description = "Ingress class name for Grafana Ingress resource."
  type        = string
  default     = "nginx"
}

variable "aws_region" {
  description = "AWS region (used for SNS endpoint URL)."
  type        = string
  default     = "us-east-1"
}
