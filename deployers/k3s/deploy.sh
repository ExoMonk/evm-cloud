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

# --- Parse handoff ---

ENGINE=$(jq -r '.compute_engine' "$HANDOFF_FILE")
MODE=$(jq -r '.mode' "$HANDOFF_FILE")
PROJECT=$(jq -r '.project_name' "$HANDOFF_FILE")

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
      --atomic --timeout 300s
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
  fi
fi

# --- Deploy workloads ---

RPC_PROXY_ENABLED=$(jq -r '.services.rpc_proxy != null' "$HANDOFF_FILE")
INDEXER_ENABLED=$(jq -r '.services.indexer != null' "$HANDOFF_FILE")

if [[ "$RPC_PROXY_ENABLED" == "true" ]]; then
  echo "[evm-cloud] Deploying eRPC (${PROJECT}-erpc)..."
  helm upgrade --install "${PROJECT}-erpc" "${CHARTS_DIR}/rpc-proxy/" \
    -f "${VALUES_DIR}/rpc-proxy-values.yaml" --rollback-on-failure --timeout 300s --create-namespace
  echo "[evm-cloud] eRPC deployed."
fi

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
      -f "$VALUES_FILE" $HELM_EXTRA_ARGS --rollback-on-failure --timeout 300s --create-namespace; then
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
