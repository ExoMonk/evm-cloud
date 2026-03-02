#!/usr/bin/env bash
# k3s Deployer — deploys workloads to a k3s cluster via Helm CLI.
# Reads from Terraform workload_handoff output (JSON) + config directory.
#
# Usage:
#   terraform output -json workload_handoff | ./deployers/k3s/deploy.sh /dev/stdin --config-dir ./config
#   # or
#   ./deployers/k3s/deploy.sh handoff.json --config-dir ./config
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CHARTS_DIR="${SCRIPT_DIR}/../charts"
SHARED_SCRIPTS="${SCRIPT_DIR}/../eks/scripts"

# --- Parse arguments ---

HANDOFF=""
CONFIG_DIR=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --config-dir) CONFIG_DIR="$2"; shift 2 ;;
    *) HANDOFF="$1"; shift ;;
  esac
done

if [[ -z "$HANDOFF" ]]; then
  echo "Usage: $0 <handoff.json> --config-dir <path>" >&2
  echo "  terraform output -json workload_handoff | $0 /dev/stdin --config-dir ./config" >&2
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

# --- Deploy workloads ---

RPC_PROXY_ENABLED=$(jq -r '.services.rpc_proxy != null' "$HANDOFF_FILE")
INDEXER_ENABLED=$(jq -r '.services.indexer != null' "$HANDOFF_FILE")

if [[ "$RPC_PROXY_ENABLED" == "true" ]]; then
  echo "[evm-cloud] Deploying eRPC (${PROJECT}-erpc)..."
  helm upgrade --install "${PROJECT}-erpc" "${CHARTS_DIR}/rpc-proxy/" \
    -f "${VALUES_DIR}/rpc-proxy-values.yaml" --wait --timeout 5m --create-namespace
  echo "[evm-cloud] eRPC deployed."
fi

if [[ "$INDEXER_ENABLED" == "true" ]]; then
  echo "[evm-cloud] Deploying rindexer (${PROJECT}-indexer)..."
  helm upgrade --install "${PROJECT}-indexer" "${CHARTS_DIR}/indexer/" \
    -f "${VALUES_DIR}/indexer-values.yaml" --wait --timeout 5m --create-namespace
  echo "[evm-cloud] rindexer deployed."
fi

echo "[evm-cloud] All workloads deployed successfully."
