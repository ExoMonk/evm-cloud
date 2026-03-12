#!/usr/bin/env bash
# k3s Deployer — deploys workloads to a k3s cluster via Helm CLI.
# Reads from Terraform workload_handoff output (JSON) + config directory.
#
# Usage:
#   terraform output -json workload_handoff | ./deployers/k3s/deploy.sh /dev/stdin --config-dir ./config
#   # or
#   ./deployers/k3s/deploy.sh handoff.json --config-dir ./config
#   # Deploy only a specific instance:
#   ./deployers/k3s/deploy.sh handoff.json --config-dir ./config --instance backfill
#   # Deploy as a one-shot Job (exits on completion, no restart):
#   ./deployers/k3s/deploy.sh handoff.json --config-dir ./config --instance backfill --job
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CHARTS_DIR="${SCRIPT_DIR}/../charts"
SHARED_SCRIPTS="${SCRIPT_DIR}/../eks/scripts"

# --- Parse arguments ---

HANDOFF=""
CONFIG_DIR=""
INSTANCE_FILTER=""
JOB_MODE=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --config-dir) CONFIG_DIR="$2"; shift 2 ;;
    --instance) INSTANCE_FILTER="$2"; shift 2 ;;
    --job) JOB_MODE=true; shift ;;
    *) HANDOFF="$1"; shift ;;
  esac
done

if [[ -z "$HANDOFF" ]]; then
  echo "Usage: $0 <handoff.json> --config-dir <path> [--instance <name>] [--job]" >&2
  echo "  terraform output -json workload_handoff | $0 /dev/stdin --config-dir ./config" >&2
  echo "  --instance <name>  Deploy only this indexer instance (e.g. backfill)" >&2
  echo "  --job              Deploy as a one-shot Job (exits on completion)" >&2
  exit 1
fi

if [[ -z "$CONFIG_DIR" ]]; then
  echo "ERROR: --config-dir is required (path to erpc.yaml, rindexer.yaml, abis/)" >&2
  exit 1
fi

for cmd in jq helm kubectl base64 python3; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "ERROR: $cmd is required but not found in PATH" >&2
    exit 1
  fi
done

# --- Buffer handoff (stdin/pipe is consumed on first read) ---

HANDOFF_FILE=$(mktemp /tmp/k3s-handoff.XXXXXX)
KUBECONFIG_PATH=$(mktemp /tmp/k3s-kubeconfig.XXXXXX)
VALUES_DIR=$(mktemp -d /tmp/k3s-values.XXXXXX)
trap "rm -rf '$HANDOFF_FILE' '$KUBECONFIG_PATH' '$VALUES_DIR'" EXIT

cat "$HANDOFF" > "$HANDOFF_FILE"
chmod 0600 "$HANDOFF_FILE"

# --- Helpers ---

# Sanitize a project name into a valid k8s namespace (DNS-1123 label):
#   lowercase, alphanumeric + hyphens, max 63 chars, no leading/trailing hyphens.
sanitize_namespace() {
  echo "$1" \
    | tr '[:upper:]' '[:lower:]' \
    | sed 's/[^a-z0-9-]/-/g' \
    | sed 's/--*/-/g' \
    | sed 's/^-//;s/-$//' \
    | cut -c1-63
}

# --- Parse handoff ---

ENGINE=$(jq -r '.compute_engine' "$HANDOFF_FILE")
MODE=$(jq -r '.mode' "$HANDOFF_FILE")
PROJECT=$(jq -r '.project_name' "$HANDOFF_FILE")
NS=$(sanitize_namespace "$PROJECT")

if [[ "$ENGINE" != "k3s" ]]; then
  echo "ERROR: handoff compute_engine must be 'k3s', got '$ENGINE'" >&2
  exit 1
fi

if [[ "$MODE" != "external" ]]; then
  echo "ERROR: handoff mode must be 'external', got '$MODE'" >&2
  exit 1
fi

# --- Render values from handoff + populate with real configs ---

echo "[evm-cloud] Rendering Helm values from handoff..."
"${SCRIPT_DIR}/scripts/render-values.sh" "$HANDOFF_FILE" "$VALUES_DIR"

echo "[evm-cloud] Populating values with configs from ${CONFIG_DIR}..."
"${SHARED_SCRIPTS}/populate-values-from-config-bundle.sh" \
  --values-dir "$VALUES_DIR" --config-dir "$CONFIG_DIR"

# --- Extract kubeconfig ---

KUBECONFIG_B64=$(jq -r '.runtime.k3s.kubeconfig_base64 // empty' "$HANDOFF_FILE")
if [[ -z "$KUBECONFIG_B64" ]]; then
  echo "ERROR: No kubeconfig_base64 found in handoff.runtime.k3s" >&2
  exit 1
fi

echo "$KUBECONFIG_B64" | base64 -d > "$KUBECONFIG_PATH"
chmod 0600 "$KUBECONFIG_PATH"
export KUBECONFIG="$KUBECONFIG_PATH"

# --- Verify cluster ---

echo "[evm-cloud] Verifying k3s cluster connectivity..."
if ! kubectl cluster-info >/dev/null 2>&1; then
  echo "ERROR: Cannot connect to k3s cluster. Check that the host is reachable and k3s is running." >&2
  exit 1
fi
echo "[evm-cloud] Cluster reachable."

# --- Cluster readiness gate ---

WORKER_COUNT=$(jq '[.runtime.k3s.worker_nodes // [] | length] | add' "$HANDOFF_FILE")
EXPECTED_NODES=$((1 + WORKER_COUNT))
READY_NODES=$(kubectl get nodes --no-headers 2>/dev/null | grep -c ' Ready' || echo 0)
if [[ "$READY_NODES" -lt "$EXPECTED_NODES" ]]; then
  echo "WARNING: Only ${READY_NODES}/${EXPECTED_NODES} nodes Ready. Pods may be Pending." >&2
fi

# --- ESO readiness gate (when secrets_mode != inline) ---

SECRETS_MODE=$(jq -r '.secrets.mode // "inline"' "$HANDOFF_FILE")

if [[ "$SECRETS_MODE" != "inline" ]]; then
  echo "[evm-cloud] Secrets mode: ${SECRETS_MODE} — checking ESO readiness..."

  # Install ESO if not already present
  ESO_CHART_VERSION=$(jq -r '.secrets.eso_chart_version // "0.9.13"' "$HANDOFF_FILE")

  if ! kubectl get crd externalsecrets.external-secrets.io >/dev/null 2>&1; then
    echo "[evm-cloud] ESO not found — installing via Helm (v${ESO_CHART_VERSION})..."
    helm repo add external-secrets https://charts.external-secrets.io 2>/dev/null || true
    helm repo update external-secrets
    helm upgrade --install external-secrets external-secrets/external-secrets \
      --namespace external-secrets --create-namespace \
      --version "$ESO_CHART_VERSION" \
      --set installCRDs=true \
      --rollback-on-failure --timeout 300s
    echo "[evm-cloud] ESO installed."
  else
    echo "[evm-cloud] ESO CRDs already present."
  fi

  # Wait for ESO CRDs to be registered
  ESO_READY=false
  for i in $(seq 1 24); do
    if kubectl get crd externalsecrets.external-secrets.io >/dev/null 2>&1; then
      ESO_READY=true
      break
    fi
    echo "[evm-cloud] Waiting for ESO CRDs... (${i}/24)"
    sleep 5
  done

  if [[ "$ESO_READY" != "true" ]]; then
    echo "ERROR: External Secrets Operator CRDs not found after 120s." >&2
    exit 1
  fi

  # Wait for ESO deployment to be ready
  kubectl -n external-secrets rollout status deployment/external-secrets --timeout=120s || {
    echo "ERROR: ESO deployment not ready after 120s." >&2
    exit 1
  }

  echo "[evm-cloud] ESO is ready."

  # Create or verify ClusterSecretStore
  if [[ "$SECRETS_MODE" == "provider" ]]; then
    SM_REGION=$(jq -r '.secrets.provider.region // "us-east-1"' "$HANDOFF_FILE")
    PROJECT_NAME=$(jq -r '.project_name' "$HANDOFF_FILE")
    STORE_NAME="${PROJECT_NAME}-aws-sm"

    echo "[evm-cloud] Creating ClusterSecretStore: ${STORE_NAME}..."
    kubectl apply -f - <<CSSEOF
apiVersion: external-secrets.io/v1beta1
kind: ClusterSecretStore
metadata:
  name: ${STORE_NAME}
spec:
  provider:
    aws:
      service: SecretsManager
      region: ${SM_REGION}
CSSEOF
    echo "[evm-cloud] ClusterSecretStore ${STORE_NAME} applied."
  elif [[ "$SECRETS_MODE" == "external" ]]; then
    EXT_STORE=$(jq -r '.secrets.external.store_name // empty' "$HANDOFF_FILE")
    if ! kubectl get clustersecretstore "$EXT_STORE" >/dev/null 2>&1; then
      echo "ERROR: ClusterSecretStore '${EXT_STORE}' not found. Create it before deploying workloads." >&2
      exit 1
    fi
    echo "[evm-cloud] ClusterSecretStore ${EXT_STORE} verified."
    STORE_NAME="$EXT_STORE"
  fi

  # Verify the referenced ClusterSecretStore is actually Ready before workload Helm installs.
  # This avoids indexer rollback timeouts when ExternalSecret cannot fetch from provider.
  STORE_READY=false
  for i in $(seq 1 24); do
    STORE_COND=$(kubectl get clustersecretstore "$STORE_NAME" -o jsonpath='{.status.conditions[?(@.type=="Ready")].status}' 2>/dev/null || true)
    if [[ "$STORE_COND" == "True" ]]; then
      STORE_READY=true
      break
    fi
    echo "[evm-cloud] Waiting for ClusterSecretStore ${STORE_NAME} to become Ready... (${i}/24)"
    sleep 5
  done

  if [[ "$STORE_READY" != "true" ]]; then
    echo "ERROR: ClusterSecretStore '${STORE_NAME}' is not Ready after 120s." >&2
    echo "  This usually means provider auth or secret access is misconfigured." >&2
    echo "  Check store status: kubectl describe clustersecretstore ${STORE_NAME}" >&2
    echo "  Check ESO logs: kubectl -n external-secrets logs deploy/external-secrets --tail=100" >&2
    exit 1
  fi
fi

# --- Ensure project namespace exists ---

kubectl create namespace "${NS}" --dry-run=client -o yaml | kubectl apply -f -

# --- Resource isolation: PriorityClasses ---

kubectl apply -f - <<PCEOF
apiVersion: scheduling.k8s.io/v1
kind: PriorityClass
metadata:
  name: evm-cloud-system
value: 1000
globalDefault: false
description: "Priority for evm-cloud core services (eRPC, indexer). Evicts custom services under pressure."
---
apiVersion: scheduling.k8s.io/v1
kind: PriorityClass
metadata:
  name: evm-cloud-custom
value: 100
globalDefault: false
description: "Priority for user-defined custom services. Evicted before core services."
PCEOF
echo "[evm-cloud] PriorityClasses applied."

# --- Ingress setup ---

INGRESS_MODE=$(jq -r '.ingress.mode // "none"' "$HANDOFF_FILE")

if [[ "$INGRESS_MODE" != "none" ]]; then
  ERPC_HOSTNAME=$(jq -r '.ingress.erpc_hostname // empty' "$HANDOFF_FILE")
  HSTS_PRELOAD=$(jq -r '.ingress.hsts_preload // false' "$HANDOFF_FILE")
  REQUEST_BODY_MAX=$(jq -r '.ingress.request_body_max_size // "1m"' "$HANDOFF_FILE")
  echo "[evm-cloud] Ingress mode: ${INGRESS_MODE} (erpc_hostname: ${ERPC_HOSTNAME})"

  # Install ingress-nginx if needed (cloudflare + ingress_nginx modes on k3s)
  if [[ "$INGRESS_MODE" == "cloudflare" || "$INGRESS_MODE" == "ingress_nginx" ]]; then
    NGINX_CHART_VERSION=$(jq -r '.ingress.nginx_chart_version // "4.11.3"' "$HANDOFF_FILE")

    if ! kubectl get ingressclass nginx >/dev/null 2>&1; then
      echo "[evm-cloud] Installing ingress-nginx (v${NGINX_CHART_VERSION})..."
      helm repo add ingress-nginx https://kubernetes.github.io/ingress-nginx 2>/dev/null || true
      helm repo update ingress-nginx

      NGINX_EXTRA_ARGS=()
      if [[ "$INGRESS_MODE" == "cloudflare" ]]; then
        # Trust Cloudflare's CF-Connecting-IP header for real client IP
        NGINX_EXTRA_ARGS+=("--set" "controller.config.use-forwarded-headers=true")
        NGINX_EXTRA_ARGS+=("--set" "controller.config.forwarded-for-header=CF-Connecting-IP")
        NGINX_EXTRA_ARGS+=("--set" "controller.config.compute-full-forwarded-for=true")
      fi

      # Create namespace first so the custom-headers ConfigMap can be applied
      kubectl create namespace ingress-nginx 2>/dev/null || true

      # Security headers via custom-headers ConfigMap (server-snippet blocked since ingress-nginx >= 4.8 / CVE-2023-5043)
      kubectl apply -f - <<HEADERSEOF
apiVersion: v1
kind: ConfigMap
metadata:
  name: ingress-nginx-custom-headers
  namespace: ingress-nginx
data:
  X-Content-Type-Options: "nosniff"
  Referrer-Policy: "strict-origin-when-cross-origin"
HEADERSEOF

      helm upgrade --install ingress-nginx ingress-nginx/ingress-nginx \
        --namespace ingress-nginx \
        --version "$NGINX_CHART_VERSION" \
        --set controller.config.hsts="true" \
        --set controller.config.hsts-max-age="31536000" \
        --set controller.config.hsts-include-subdomains="true" \
        --set controller.config.hsts-preload="${HSTS_PRELOAD}" \
        --set controller.config.hide-headers="Server" \
        --set controller.config.ssl-protocols="TLSv1.2 TLSv1.3" \
        --set controller.config.proxy-body-size="${REQUEST_BODY_MAX}" \
        --set controller.config.add-headers="ingress-nginx/ingress-nginx-custom-headers" \
        --set controller.hostPort.enabled=true \
        --set controller.service.type=ClusterIP \
        --set-string controller.nodeSelector."node-role\.kubernetes\.io/master"=true \
        --set controller.tolerations[0].key="node-role.kubernetes.io/master" \
        --set controller.tolerations[0].effect="NoSchedule" \
        "${NGINX_EXTRA_ARGS[@]}" \
        --rollback-on-failure --timeout 300s
      echo "[evm-cloud] ingress-nginx installed."
    else
      echo "[evm-cloud] ingress-nginx already present."
    fi
  fi

  # cert-manager: install + create ClusterIssuer (ingress_nginx mode only)
  if [[ "$INGRESS_MODE" == "ingress_nginx" ]]; then
    CERT_MANAGER_VERSION=$(jq -r '.ingress.cert_manager_chart_version // "1.16.2"' "$HANDOFF_FILE")
    TLS_EMAIL=$(jq -r '.ingress.tls_email // empty' "$HANDOFF_FILE")
    TLS_STAGING=$(jq -r '.ingress.tls_staging // false' "$HANDOFF_FILE")

    if ! kubectl get crd certificates.cert-manager.io >/dev/null 2>&1; then
      echo "[evm-cloud] Installing cert-manager (v${CERT_MANAGER_VERSION})..."
      helm repo add jetstack https://charts.jetstack.io 2>/dev/null || true
      helm repo update jetstack
      helm upgrade --install cert-manager jetstack/cert-manager \
        --namespace cert-manager --create-namespace \
        --version "$CERT_MANAGER_VERSION" \
        --set crds.enabled=true \
        --rollback-on-failure --timeout 300s
      echo "[evm-cloud] cert-manager installed."
    else
      echo "[evm-cloud] cert-manager CRDs already present."
    fi

    # Wait for cert-manager webhook to be ready
    kubectl -n cert-manager rollout status deployment/cert-manager-webhook --timeout=120s || {
      echo "ERROR: cert-manager webhook not ready after 120s." >&2
      exit 1
    }

    # Create ClusterIssuer(s) for Let's Encrypt
    ACME_SERVER="https://acme-v02.api.letsencrypt.org/directory"
    ISSUER_NAME="letsencrypt-prod"
    if [[ "$TLS_STAGING" == "true" ]]; then
      ACME_SERVER="https://acme-staging-v02.api.letsencrypt.org/directory"
      ISSUER_NAME="letsencrypt-staging"
    fi

    echo "[evm-cloud] Creating ClusterIssuer: ${ISSUER_NAME}..."

    # Write manifest to temp file for retry loop
    ISSUER_MANIFEST=$(mktemp /tmp/cluster-issuer.XXXXXX.yaml)
    cat > "$ISSUER_MANIFEST" <<ISSUEREOF
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: ${ISSUER_NAME}
spec:
  acme:
    server: ${ACME_SERVER}
    email: ${TLS_EMAIL}
    privateKeySecretRef:
      name: ${ISSUER_NAME}-account-key
    solvers:
      - http01:
          ingress:
            class: nginx
ISSUEREOF

    # Retry loop: webhook admission registration can lag behind deployment readiness
    for _attempt in $(seq 1 6); do
      if kubectl apply -f "$ISSUER_MANIFEST" 2>/dev/null; then
        echo "[evm-cloud] ClusterIssuer ${ISSUER_NAME} applied."
        break
      fi
      if [ "$_attempt" -eq 6 ]; then
        echo "ERROR: cert-manager webhook not accepting ClusterIssuer after 30s." >&2
        rm -f "$ISSUER_MANIFEST"
        exit 1
      fi
      echo "  Waiting for cert-manager webhook (attempt $_attempt/6)..."
      sleep 5
    done
    rm -f "$ISSUER_MANIFEST"
  fi

  # Cloudflare mode: create TLS secret from origin cert
  if [[ "$INGRESS_MODE" == "cloudflare" ]]; then
    CF_ORIGIN_CERT=$(jq -r '.ingress.cloudflare.origin_cert // empty' "$HANDOFF_FILE")
    CF_ORIGIN_KEY=$(jq -r '.ingress.cloudflare.origin_key // empty' "$HANDOFF_FILE")

    if [[ -z "$CF_ORIGIN_CERT" || -z "$CF_ORIGIN_KEY" ]]; then
      echo "ERROR: Cloudflare origin cert/key missing from handoff.ingress.cloudflare" >&2
      exit 1
    fi

    echo "[evm-cloud] Creating Cloudflare origin TLS secret..."
    CF_TLS_YAML=$(kubectl create secret tls cloudflare-origin-tls \
      --cert=<(echo "$CF_ORIGIN_CERT") \
      --key=<(echo "$CF_ORIGIN_KEY") \
      --dry-run=client -o yaml)
    # Apply to project namespace (for eRPC ingress)
    echo "$CF_TLS_YAML" | kubectl apply -n "${NS}" -f -
    # Apply to monitoring namespace (for Grafana ingress)
    kubectl create namespace monitoring 2>/dev/null || true
    echo "$CF_TLS_YAML" | kubectl apply -n monitoring -f -
    echo "[evm-cloud] Cloudflare origin TLS secret created (${NS} + monitoring)."
  fi
fi

# --- Monitoring stack ---

MONITORING_ENABLED=$(jq -r '.services.monitoring != null' "$HANDOFF_FILE")

if [[ "$MONITORING_ENABLED" == "true" ]]; then
  KUBE_PROM_VERSION=$(jq -r '.services.monitoring.kube_prometheus_stack_version // "72.6.2"' "$HANDOFF_FILE")
  GRAFANA_ADMIN_SECRET=$(jq -r '.services.monitoring.grafana_admin_password_secret_name // empty' "$HANDOFF_FILE")
  AM_ROUTE_TARGET=$(jq -r '.services.monitoring.alertmanager_route_target // "slack"' "$HANDOFF_FILE")
  AM_SLACK_SECRET=$(jq -r '.services.monitoring.alertmanager_slack_webhook_secret_name // empty' "$HANDOFF_FILE")
  AM_SLACK_CHANNEL=$(jq -r '.services.monitoring.alertmanager_slack_channel // "#alerts"' "$HANDOFF_FILE")
  AM_SNS_ARN=$(jq -r '.services.monitoring.alertmanager_sns_topic_arn // empty' "$HANDOFF_FILE")
  AM_PD_SECRET=$(jq -r '.services.monitoring.alertmanager_pagerduty_routing_key_secret_name // empty' "$HANDOFF_FILE")
  LOKI_ENABLED=$(jq -r '.services.monitoring.loki_enabled // false' "$HANDOFF_FILE")
  LOKI_VERSION=$(jq -r '.services.monitoring.loki_chart_version // "6.24.0"' "$HANDOFF_FILE")
  PROMTAIL_VERSION=$(jq -r '.services.monitoring.promtail_chart_version // "6.16.6"' "$HANDOFF_FILE")
  LOKI_PERSISTENCE=$(jq -r '.services.monitoring.loki_persistence_enabled // false' "$HANDOFF_FILE")
  CH_METRICS_URL=$(jq -r '.services.monitoring.clickhouse_metrics_url // empty' "$HANDOFF_FILE")
  CH_GRAFANA_URL=$(jq -r '.data.clickhouse.url // empty' "$HANDOFF_FILE")
  # Parse host:port from URL formats: "clickhouse://host:port/db", "host:port", "host"
  CH_STRIPPED=$(echo "$CH_GRAFANA_URL" | sed -E 's|^[a-z]+://||')   # strip scheme
  CH_STRIPPED=$(echo "$CH_STRIPPED" | sed -E 's|/.*$||')             # strip path
  CH_GRAFANA_HOST=$(echo "$CH_STRIPPED" | sed -E 's|:[0-9]+$||')    # strip port
  CH_GRAFANA_PORT=$(echo "$CH_STRIPPED" | sed -nE 's|.*:([0-9]+)$|\1|p')
  CH_GRAFANA_PORT=${CH_GRAFANA_PORT:-9000}
  CH_GRAFANA_USER=$(jq -r '.data.clickhouse.user // "default"' "$HANDOFF_FILE")
  CH_GRAFANA_DB=$(jq -r '.data.clickhouse.db // "default"' "$HANDOFF_FILE")
  CH_GRAFANA_PASSWORD=$(jq -r '.data.clickhouse.password // empty' "$HANDOFF_FILE")
  if [[ -n "$CH_GRAFANA_HOST" ]]; then
    echo "[evm-cloud] ClickHouse datasource: host=$CH_GRAFANA_HOST port=$CH_GRAFANA_PORT db=$CH_GRAFANA_DB user=$CH_GRAFANA_USER"
  fi
  GRAFANA_INGRESS=$(jq -r '.services.monitoring.grafana_ingress_enabled // true' "$HANDOFF_FILE")
  GRAFANA_HOSTNAME=$(jq -r '.services.monitoring.grafana_hostname // empty' "$HANDOFF_FILE")
  GRAFANA_INGRESS_CLASS=$(jq -r '.services.monitoring.ingress_class_name // "nginx"' "$HANDOFF_FILE")

  # Memory gate: skip monitoring on nodes with <1.5GB allocatable unless forced
  # Monitoring stack requests ~300Mi total (Prometheus 128Mi + Grafana 64Mi + exporters).
  # A t3.small (2GB) has ~1.7GB allocatable — fits lightweight workloads + monitoring.
  if [[ "${FORCE_MONITORING:-}" != "true" ]]; then
    ALLOC_MEM_KI=$(kubectl get nodes -o jsonpath='{.items[0].status.allocatable.memory}' 2>/dev/null | sed 's/Ki//')
    ALLOC_MEM_MB=$((ALLOC_MEM_KI / 1024))
    if [[ "$ALLOC_MEM_MB" -lt 1536 ]]; then
      echo "WARNING: Node has <1.5GB allocatable memory (~${ALLOC_MEM_MB}Mi). Skipping monitoring." >&2
      echo "  Set FORCE_MONITORING=true to override." >&2
      MONITORING_ENABLED="false"
    fi
  fi
fi

if [[ "$MONITORING_ENABLED" == "true" ]]; then
  kubectl create namespace monitoring 2>/dev/null || true

  # Multi-project guard: skip if monitoring was installed by a different project.
  # Same project redeploys always upgrade (picks up new datasources, dashboards, etc.).
  MONITORING_OWNER=""
  if helm status monitoring -n monitoring >/dev/null 2>&1; then
    MONITORING_OWNER=$(helm get values monitoring -n monitoring -o json 2>/dev/null | jq -r '.evmCloudOwner // empty')
  fi
  if [[ -n "$MONITORING_OWNER" && "$MONITORING_OWNER" != "$PROJECT" ]]; then
    echo "[evm-cloud] kube-prometheus-stack owned by project '$MONITORING_OWNER', skipping."
  else
  echo "[evm-cloud] Installing/upgrading kube-prometheus-stack (v${KUBE_PROM_VERSION})..."

  helm repo add prometheus-community https://prometheus-community.github.io/helm-charts 2>/dev/null || true
  helm repo update prometheus-community

  PROM_ARGS=(
    --namespace monitoring
    --version "$KUBE_PROM_VERSION"
    --set "evmCloudOwner=$PROJECT"
    --set prometheus.prometheusSpec.serviceMonitorSelectorNilUsesHelmValues=false
    --set prometheus.prometheusSpec.retention=7d
    --set prometheus.prometheusSpec.resources.requests.cpu=100m
    --set prometheus.prometheusSpec.resources.requests.memory=128Mi
    --set prometheus.prometheusSpec.resources.limits.memory=512Mi
    --set grafana.sidecar.dashboards.enabled=true
    --set grafana.sidecar.dashboards.searchNamespace=ALL
    --set grafana.resources.requests.cpu=50m
    --set grafana.resources.requests.memory=64Mi
    --set grafana.resources.limits.memory=196Mi
  )

  # Grafana admin secret
  if [[ -n "$GRAFANA_ADMIN_SECRET" ]]; then
    PROM_ARGS+=(--set grafana.admin.existingSecret="$GRAFANA_ADMIN_SECRET")
  fi

  # Grafana ingress
  if [[ "$GRAFANA_INGRESS" == "true" && -n "$GRAFANA_HOSTNAME" ]]; then
    PROM_ARGS+=(
      --set grafana.ingress.enabled=true
      --set grafana.ingress.ingressClassName="$GRAFANA_INGRESS_CLASS"
      --set "grafana.ingress.hosts[0]=$GRAFANA_HOSTNAME"
      --set "grafana.\"grafana\\.ini\".server.root_url=https://$GRAFANA_HOSTNAME"
    )
    # TLS for Grafana ingress
    if [[ "$INGRESS_MODE" == "ingress_nginx" ]]; then
      PROM_ARGS+=(
        --set "grafana.ingress.tls[0].secretName=grafana-tls"
        --set "grafana.ingress.tls[0].hosts[0]=$GRAFANA_HOSTNAME"
        --set "grafana.ingress.annotations.cert-manager\\.io/cluster-issuer=${ISSUER_NAME:-letsencrypt-prod}"
      )
    elif [[ "$INGRESS_MODE" == "cloudflare" ]]; then
      PROM_ARGS+=(
        --set "grafana.ingress.tls[0].secretName=cloudflare-origin-tls"
        --set "grafana.ingress.tls[0].hosts[0]=$GRAFANA_HOSTNAME"
      )
    fi
  fi

  # Additional datasources in Grafana (indexed sequentially)
  DS_IDX=0

  # Loki datasource
  if [[ "$LOKI_ENABLED" == "true" ]]; then
    PROM_ARGS+=(
      --set "grafana.additionalDataSources[$DS_IDX].name=Loki"
      --set "grafana.additionalDataSources[$DS_IDX].type=loki"
      --set "grafana.additionalDataSources[$DS_IDX].url=http://loki.monitoring.svc:3100"
      --set "grafana.additionalDataSources[$DS_IDX].access=proxy"
    )
    DS_IDX=$((DS_IDX + 1))
  fi

  # ClickHouse datasource (for template dashboards)
  # Detect protocol from URL scheme + port:
  #   clickhouse+ssl:// or port 8443/9440 → native + secure
  #   https://           or port 8443     → http + secure
  #   clickhouse://      or port 9000     → native
  #   http://            or port 8123     → http
  if [[ -n "$CH_GRAFANA_HOST" ]]; then
    CH_PROTO="native"
    CH_SECURE="false"
    case "$CH_GRAFANA_URL" in
      https://*) CH_PROTO="http"; CH_SECURE="true" ;;
      http://*)  CH_PROTO="http" ;;
    esac
    # Port-based TLS detection (ClickHouse Cloud uses 8443 for HTTPS)
    if [[ "$CH_GRAFANA_PORT" == "8443" ]]; then
      CH_PROTO="http"; CH_SECURE="true"
    elif [[ "$CH_GRAFANA_PORT" == "9440" ]]; then
      CH_PROTO="native"; CH_SECURE="true"
    elif [[ "$CH_GRAFANA_PORT" == "8123" ]]; then
      CH_PROTO="http"
    fi
    echo "[evm-cloud] ClickHouse Grafana datasource: protocol=$CH_PROTO secure=$CH_SECURE"

    PROM_ARGS+=(
      --set "grafana.plugins[0]=grafana-clickhouse-datasource"
      --set-string "grafana.additionalDataSources[$DS_IDX].name=ClickHouse"
      --set-string "grafana.additionalDataSources[$DS_IDX].type=grafana-clickhouse-datasource"
      --set-string "grafana.additionalDataSources[$DS_IDX].uid=clickhouse"
      --set-string "grafana.additionalDataSources[$DS_IDX].access=proxy"
      --set-string "grafana.additionalDataSources[$DS_IDX].jsonData.host=$CH_GRAFANA_HOST"
      --set "grafana.additionalDataSources[$DS_IDX].jsonData.port=$CH_GRAFANA_PORT"
      --set-string "grafana.additionalDataSources[$DS_IDX].jsonData.protocol=$CH_PROTO"
      --set "grafana.additionalDataSources[$DS_IDX].jsonData.secure=$CH_SECURE"
      --set-string "grafana.additionalDataSources[$DS_IDX].jsonData.username=$CH_GRAFANA_USER"
      --set-string "grafana.additionalDataSources[$DS_IDX].jsonData.defaultDatabase=$CH_GRAFANA_DB"
      --set-string "grafana.additionalDataSources[$DS_IDX].secureJsonData.password=$CH_GRAFANA_PASSWORD"
    )
    DS_IDX=$((DS_IDX + 1))
  fi

  # Alertmanager routing
  if [[ "$AM_ROUTE_TARGET" == "slack" && -n "$AM_SLACK_SECRET" ]]; then
    PROM_ARGS+=(
      --set alertmanager.config.route.receiver=slack
      --set alertmanager.config.route.routes[0].match.severity=critical
      --set alertmanager.config.route.routes[0].receiver=slack
    )
  fi

  # ClickHouse BYO scrape
  if [[ -n "$CH_METRICS_URL" ]]; then
    PROM_ARGS+=(
      --set "prometheus.prometheusSpec.additionalScrapeConfigs[0].job_name=clickhouse"
      --set "prometheus.prometheusSpec.additionalScrapeConfigs[0].static_configs[0].targets[0]=$CH_METRICS_URL"
    )
  fi

  helm upgrade --install monitoring prometheus-community/kube-prometheus-stack \
    "${PROM_ARGS[@]}" \
    --rollback-on-failure --timeout 600s
  echo "[evm-cloud] kube-prometheus-stack ready."
  fi # end multi-project guard

  # Optional: Loki + Promtail
  if [[ "$LOKI_ENABLED" == "true" ]]; then
    if helm status loki -n monitoring >/dev/null 2>&1; then
      echo "[evm-cloud] Loki already present."
    else
      echo "[evm-cloud] Installing Loki (v${LOKI_VERSION})..."
      helm repo add grafana https://grafana.github.io/helm-charts 2>/dev/null || true
      helm repo update grafana

      LOKI_ARGS=(
        --namespace monitoring
        --version "$LOKI_VERSION"
        --set deploymentMode=SingleBinary
        --set loki.auth_enabled=false
        --set singleBinary.replicas=1
        --set loki.commonConfig.replication_factor=1
        --set loki.storage.type=filesystem
        --set singleBinary.persistence.enabled="$LOKI_PERSISTENCE"
        --set read.replicas=0
        --set write.replicas=0
        --set backend.replicas=0
      )

      helm upgrade --install loki grafana/loki \
        "${LOKI_ARGS[@]}" \
        --rollback-on-failure --timeout 300s
      echo "[evm-cloud] Loki installed."
    fi

    if helm status promtail -n monitoring >/dev/null 2>&1; then
      echo "[evm-cloud] Promtail already present."
    else
      echo "[evm-cloud] Installing Promtail (v${PROMTAIL_VERSION})..."
      helm upgrade --install promtail grafana/promtail \
        --namespace monitoring \
        --version "$PROMTAIL_VERSION" \
        --set "config.clients[0].url=http://loki.monitoring.svc:3100/loki/api/v1/push" \
        --rollback-on-failure --timeout 300s
      echo "[evm-cloud] Promtail installed."
    fi
  fi

  # Deploy dashboards + alert rules chart
  echo "[evm-cloud] Deploying dashboards and alert rules..."
  helm upgrade --install "${PROJECT}-dashboards" "${CHARTS_DIR}/dashboards/" \
    --namespace monitoring \
    --set projectName="$PROJECT" \
    --set workloadNamespace="$NS" \
    --set monitoringRelease=monitoring \
    --timeout 120s
  echo "[evm-cloud] Dashboards deployed."

  # Deploy custom dashboards from grafana/ directory (e.g. from templates apply)
  GRAFANA_DIR="$(dirname "$CONFIG_DIR")/grafana"
  if [[ -d "$GRAFANA_DIR" ]]; then
    echo "[evm-cloud] Deploying custom dashboards from ${GRAFANA_DIR}..."
    for dashboard_file in "$GRAFANA_DIR"/*.json; do
      [[ -f "$dashboard_file" ]] || continue
      fname="$(basename "$dashboard_file")"
      cm_name="${PROJECT}-$(echo "${fname%.json}" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9-]/-/g')"
      kubectl apply -n monitoring -f - <<CMEOF
apiVersion: v1
kind: ConfigMap
metadata:
  name: ${cm_name}
  namespace: monitoring
  labels:
    grafana_dashboard: "1"
data:
  ${fname}: |
$(sed 's/^/    /' "$dashboard_file")
CMEOF
    done
    echo "[evm-cloud] Custom dashboards deployed."
  fi
fi

# --- Deploy workloads ---

RPC_PROXY_ENABLED=$(jq -r '.services.rpc_proxy != null' "$HANDOFF_FILE")
INDEXER_ENABLED=$(jq -r '.services.indexer != null' "$HANDOFF_FILE")

# --- Indexer config sanity checks ---

if [[ "$INDEXER_ENABLED" == "true" ]]; then
  # Default scaffolded config has `contracts: []`; deploying it causes crashlooping pods.
  # Fail early with a clear message instead of waiting for runtime crashes.
  if [[ -f "$CONFIG_DIR/rindexer.yaml" ]] && grep -Eq '^[[:space:]]*contracts:[[:space:]]*\[[[:space:]]*\][[:space:]]*$' "$CONFIG_DIR/rindexer.yaml"; then
    echo "ERROR: rindexer config has an empty contracts list in ${CONFIG_DIR}/rindexer.yaml" >&2
    echo "  Add at least one contract to index, then redeploy." >&2
    echo "  Or disable indexer for this stack if you only want eRPC/infra." >&2
    exit 1
  fi
fi

if [[ "$RPC_PROXY_ENABLED" == "true" ]]; then
  echo "[evm-cloud] Deploying eRPC (${PROJECT}-erpc)..."
  helm upgrade --install "${PROJECT}-erpc" "${CHARTS_DIR}/rpc-proxy/" \
    -n "${NS}" -f "${VALUES_DIR}/rpc-proxy-values.yaml" --rollback-on-failure --timeout 300s
  echo "[evm-cloud] eRPC deployed."
fi

# --- Deploy custom services (before indexer — indexer webhooks may target custom services) ---

CUSTOM_SERVICES_JSON=$(jq -c '.services.custom_services // null' "$HANDOFF_FILE")

if [[ "$CUSTOM_SERVICES_JSON" != "null" && "$CUSTOM_SERVICES_JSON" != "[]" ]]; then
  # ResourceQuota: cap aggregate custom service consumption
  kubectl apply -f - <<RQEOF
apiVersion: v1
kind: ResourceQuota
metadata:
  name: custom-services-quota
  namespace: ${NS}
spec:
  hard:
    requests.cpu: "2"
    requests.memory: 2Gi
    limits.cpu: "4"
    limits.memory: 4Gi
RQEOF
  echo "[evm-cloud] ResourceQuota for custom services applied."

  CUSTOM_DEPLOY_FAILED=0

  for SVC in $(echo "$CUSTOM_SERVICES_JSON" | jq -c '.[]'); do
    SVC_NAME=$(echo "$SVC" | jq -r '.name')
    VALUES_FILE="${VALUES_DIR}/custom-${SVC_NAME}-values.yaml"

    if [[ ! -f "$VALUES_FILE" ]]; then
      echo "ERROR: Values file not found for custom service '${SVC_NAME}': ${VALUES_FILE}" >&2
      CUSTOM_DEPLOY_FAILED=1
      continue
    fi

    echo "[evm-cloud] Deploying custom service (${PROJECT}-${SVC_NAME})..."
    if ! helm upgrade --install "${PROJECT}-${SVC_NAME}" "${CHARTS_DIR}/custom-service/" \
      -n "${NS}" -f "$VALUES_FILE" --rollback-on-failure --timeout 300s; then
      echo "ERROR: Failed to deploy custom service ${PROJECT}-${SVC_NAME}" >&2
      CUSTOM_DEPLOY_FAILED=1
    else
      echo "[evm-cloud] ${PROJECT}-${SVC_NAME} deployed."
    fi
  done

  if [[ "$CUSTOM_DEPLOY_FAILED" -ne 0 ]]; then
    echo "ERROR: One or more custom services failed to deploy." >&2
    exit 1
  fi
fi

# --- Deploy indexer (after custom services so webhook targets are reachable) ---

if [[ "$INDEXER_ENABLED" == "true" ]]; then
  # Multi-instance support: loop over instances[] if present, fallback to single release
  INSTANCES=$(jq -c '.services.indexer.instances // [{"name":"indexer","config_key":"default"}]' "$HANDOFF_FILE")
  DEPLOY_FAILED=0

  for INSTANCE in $(echo "$INSTANCES" | jq -c '.[]'); do
    NAME=$(echo "$INSTANCE" | jq -r '.name')

    # --instance filter: skip instances that don't match
    if [[ -n "$INSTANCE_FILTER" && "$NAME" != "$INSTANCE_FILTER" ]]; then
      echo "[evm-cloud] Skipping ${PROJECT}-${NAME} (filtered by --instance ${INSTANCE_FILTER})"
      continue
    fi

    VALUES_FILE="${VALUES_DIR}/${NAME}-values.yaml"

    # Fallback: if per-instance values file doesn't exist, use the default indexer-values.yaml
    if [[ ! -f "$VALUES_FILE" ]]; then
      VALUES_FILE="${VALUES_DIR}/indexer-values.yaml"
    fi

    HELM_EXTRA_ARGS=""
    if [[ "$JOB_MODE" == "true" ]]; then
      HELM_EXTRA_ARGS="--set workloadType=job"
    fi

    echo "[evm-cloud] Deploying rindexer instance (${PROJECT}-${NAME})..."
    if ! helm upgrade --install "${PROJECT}-${NAME}" "${CHARTS_DIR}/indexer/" \
      -n "${NS}" -f "$VALUES_FILE" $HELM_EXTRA_ARGS --rollback-on-failure --timeout 300s; then
      echo "ERROR: Failed to deploy ${PROJECT}-${NAME}" >&2
      DEPLOY_FAILED=1
    else
      echo "[evm-cloud] ${PROJECT}-${NAME} deployed."
    fi
  done

  if [[ "$DEPLOY_FAILED" -ne 0 ]]; then
    echo "ERROR: One or more indexer instances failed to deploy." >&2
    exit 1
  fi
fi

# --- Post-deploy: verify ExternalSecret sync (non-inline modes) ---
if [[ "$SECRETS_MODE" != "inline" ]]; then
  echo "[evm-cloud] Verifying ExternalSecret sync status..."
  SYNC_TIMEOUT=60
  SYNC_ELAPSED=0
  ALL_SYNCED=false
  while [[ $SYNC_ELAPSED -lt $SYNC_TIMEOUT ]]; do
    NOT_SYNCED=$(kubectl get externalsecret -A -o jsonpath='{range .items[*]}{.metadata.name}={.status.conditions[?(@.type=="Ready")].status}{"\n"}{end}' 2>/dev/null | grep -v "=True" | grep -v "^$" || true)
    if [[ -z "$NOT_SYNCED" ]]; then
      ALL_SYNCED=true
      break
    fi
    sleep 5
    SYNC_ELAPSED=$((SYNC_ELAPSED + 5))
  done
  if [[ "$ALL_SYNCED" == "true" ]]; then
    echo "[evm-cloud] All ExternalSecrets synced successfully."
  else
    echo "WARNING: Some ExternalSecrets not synced after ${SYNC_TIMEOUT}s. Pods may fail to start." >&2
    echo "  Check: kubectl get externalsecret -A" >&2
    echo "  Force sync: kubectl annotate es <name> force-sync=\$(date +%s)" >&2
  fi
fi

echo "[evm-cloud] All workloads deployed successfully."
