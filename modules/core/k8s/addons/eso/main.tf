# External Secrets Operator — syncs secrets from external stores into K8s Secrets.
# ClusterSecretStore is NOT managed here — deploy.sh creates it at deploy time
# to avoid kubernetes_manifest CRD-at-plan-time issues.

resource "helm_release" "external_secrets" {
  count = var.enabled ? 1 : 0

  name             = "external-secrets"
  repository       = "https://charts.external-secrets.io"
  chart            = "external-secrets"
  version          = var.eso_chart_version
  namespace        = "external-secrets"
  create_namespace = true
  atomic           = true
  timeout          = 300

  set {
    name  = "installCRDs"
    value = "true"
  }
}
