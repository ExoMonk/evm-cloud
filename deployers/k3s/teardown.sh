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

# Sanitize a project name into a valid k8s namespace (DNS-1123 label)
sanitize_namespace() {
  echo "$1" \
    | tr '[:upper:]' '[:lower:]' \
    | sed 's/[^a-z0-9-]/-/g' \
    | sed 's/--*/-/g' \
    | sed 's/^-//;s/-$//' \
    | cut -c1-63
}

PROJECT=$(jq -r '.project_name' "$HANDOFF_FILE")
NS=$(sanitize_namespace "$PROJECT")

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

# Uninstall eRPC
helm uninstall "${PROJECT}-erpc" -n "${NS}" --wait 2>/dev/null && echo "  Removed ${PROJECT}-erpc" || true

# Uninstall indexer instances (multi-instance aware)
INSTANCES=$(jq -c '.services.indexer.instances // [{"name":"indexer"}]' "$HANDOFF_FILE")
for INSTANCE in $(echo "$INSTANCES" | jq -c '.[]'); do
  NAME=$(echo "$INSTANCE" | jq -r '.name')
  helm uninstall "${PROJECT}-${NAME}" -n "${NS}" --wait 2>/dev/null && echo "  Removed ${PROJECT}-${NAME}" || true
done

# Clean up ingress resources (best-effort, regardless of current ingress mode)
INGRESS_MODE=$(jq -r '.ingress.mode // "none"' "$HANDOFF_FILE")
echo "[evm-cloud] Cleaning ingress resources (mode: $INGRESS_MODE)..."

# Remove Cloudflare TLS secret from project + monitoring namespaces
kubectl delete secret cloudflare-origin-tls -n "${NS}" 2>/dev/null && echo "  Removed cloudflare-origin-tls secret (${NS})" || true

# Remove cert-manager issuers first (cluster-scoped)
kubectl delete clusterissuer letsencrypt-prod letsencrypt-staging 2>/dev/null || true

# Uninstall cert-manager (if present)
helm uninstall cert-manager -n cert-manager --wait 2>/dev/null && echo "  Removed cert-manager" || true

# Uninstall ingress-nginx (if present)
helm uninstall ingress-nginx -n ingress-nginx --wait 2>/dev/null && echo "  Removed ingress-nginx" || true

# Remove leftover namespaced resources and namespaces (ignore not found)
kubectl delete configmap ingress-nginx-custom-headers -n ingress-nginx 2>/dev/null || true
kubectl delete namespace cert-manager --ignore-not-found=true 2>/dev/null || true
kubectl delete namespace ingress-nginx --ignore-not-found=true 2>/dev/null || true

# Remove the project namespace (all remaining resources within it will be garbage collected)
kubectl delete namespace "${NS}" --ignore-not-found=true 2>/dev/null || true

echo "[evm-cloud] Teardown complete. Safe to run 'terraform destroy'."
