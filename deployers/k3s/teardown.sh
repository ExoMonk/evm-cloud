#!/usr/bin/env bash
# k3s Teardown — uninstalls all Helm releases deployed by deploy.sh.
# Run this BEFORE terraform destroy for clean teardown.
#
# Usage:
#   terraform output -json workload_handoff | ./deployers/k3s/teardown.sh /dev/stdin
set -euo pipefail

HANDOFF="${1:-}"
if [[ -z "$HANDOFF" ]]; then
  echo "Usage: $0 <handoff.json>" >&2
  exit 1
fi

for cmd in jq helm kubectl base64; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "ERROR: $cmd is required but not found in PATH" >&2
    exit 1
  fi
done

# Buffer handoff (stdin/pipe is consumed on first read)
HANDOFF_FILE=$(mktemp /tmp/k3s-handoff.XXXXXX)
KUBECONFIG_PATH=$(mktemp /tmp/k3s-kubeconfig.XXXXXX)
trap "rm -f '$HANDOFF_FILE' '$KUBECONFIG_PATH'" EXIT

cat "$HANDOFF" > "$HANDOFF_FILE"

PROJECT=$(jq -r '.project_name' "$HANDOFF_FILE")

# Extract kubeconfig
KUBECONFIG_B64=$(jq -r '.runtime.k3s.kubeconfig_base64 // empty' "$HANDOFF_FILE")
if [[ -z "$KUBECONFIG_B64" ]]; then
  echo "ERROR: No kubeconfig_base64 found in handoff" >&2
  exit 1
fi

echo "$KUBECONFIG_B64" | base64 -d > "$KUBECONFIG_PATH"
chmod 0600 "$KUBECONFIG_PATH"
export KUBECONFIG="$KUBECONFIG_PATH"

echo "[evm-cloud] Tearing down workloads..."

# Uninstall known releases (ignore errors if not found)
helm uninstall "${PROJECT}-erpc" --wait 2>/dev/null && echo "  Removed ${PROJECT}-erpc" || true
helm uninstall "${PROJECT}-indexer" --wait 2>/dev/null && echo "  Removed ${PROJECT}-indexer" || true

echo "[evm-cloud] Teardown complete. Safe to run 'terraform destroy'."
