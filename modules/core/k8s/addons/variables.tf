variable "project_name" {
  description = "Project name prefix for Helm release naming."
  type        = string
}

variable "namespace" {
  description = "Default Kubernetes namespace for addon deployments."
  type        = string
  default     = "addons"
}

variable "eso_enabled" {
  description = "Enable External Secrets Operator addon."
  type        = bool
  default     = false
}

variable "eso_chart_version" {
  description = "External Secrets Operator Helm chart version."
  type        = string
  default     = "0.9.13"
}

variable "eso_service_account_role_arn" {
  description = "IRSA role ARN for ESO ServiceAccount annotation (EKS only)."
  type        = string
  default     = ""
}

# --- Monitoring ---

variable "monitoring_enabled" {
  description = "Enable kube-prometheus-stack monitoring addon."
  type        = bool
  default     = false
}

variable "kube_prometheus_stack_version" {
  description = "kube-prometheus-stack Helm chart version."
  type        = string
  default     = "72.6.2"
}

variable "grafana_admin_password_secret_name" {
  description = "Existing K8s Secret name for Grafana admin password."
  type        = string
  default     = ""
}

variable "alertmanager_slack_webhook_secret_name" {
  description = "Existing K8s Secret name holding Slack webhook URL."
  type        = string
  default     = ""
}

variable "alertmanager_sns_topic_arn" {
  description = "SNS topic ARN for Alertmanager webhook routing."
  type        = string
  default     = ""
}

variable "alertmanager_pagerduty_routing_key_secret_name" {
  description = "Existing K8s Secret name holding PagerDuty routing key."
  type        = string
  default     = ""
}

variable "alertmanager_route_target" {
  description = "Alertmanager receiver target: slack, sns, or pagerduty."
  type        = string
  default     = "slack"
}

variable "alertmanager_slack_channel" {
  description = "Slack channel name for alert delivery."
  type        = string
  default     = "#alerts"
}

variable "loki_enabled" {
  description = "Deploy Loki + Promtail for log aggregation."
  type        = bool
  default     = false
}

variable "loki_chart_version" {
  description = "Loki Helm chart version."
  type        = string
  default     = "6.24.0"
}

variable "promtail_chart_version" {
  description = "Promtail Helm chart version."
  type        = string
  default     = "6.16.6"
}

variable "loki_persistence_enabled" {
  description = "Enable PVC for Loki."
  type        = bool
  default     = false
}

variable "clickhouse_metrics_url" {
  description = "Optional BYO ClickHouse metrics endpoint."
  type        = string
  default     = ""
}

variable "grafana_ingress_enabled" {
  description = "Expose Grafana via Ingress."
  type        = bool
  default     = true
}

variable "grafana_hostname" {
  description = "Grafana hostname for Ingress."
  type        = string
  default     = ""
}

variable "grafana_extra_dashboards" {
  description = "Map of dashboard filename to JSON content for Grafana sidecar loading."
  type        = map(string)
  default     = {}
}

variable "ingress_class_name" {
  description = "Ingress class name for Grafana."
  type        = string
  default     = "nginx"
}

variable "aws_region" {
  description = "AWS region (used for SNS endpoint URL)."
  type        = string
  default     = "us-east-1"
}
