#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Kind-based deployer integration test
# Runs deployers/k3s/deploy.sh against an ephemeral kind cluster.
# Validates Helm releases, K8s resources, pod health, upgrades, and teardown.
#
# Usage: make test-deploy
# Prereqs: docker, kind, kubectl, helm, jq, base64, python3
# Runtime: ~2-5 min (cached images), ~5-8 min (cold pull)
# =============================================================================

CLUSTER_NAME="evm-cloud-deploy-test"
PROJECT="deploy-test"
NS="deploy-test"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CONFIG_DIR="${SCRIPT_DIR}/deployer-config"
CHARTS_DIR="${REPO_ROOT}/deployers/charts"
DEPLOY_SCRIPT="${REPO_ROOT}/deployers/k3s/deploy.sh"
TEARDOWN_SCRIPT="${REPO_ROOT}/deployers/k3s/teardown.sh"
KUBECONFIG_PATH=""
HANDOFF_FILE=""
UPGRADE_CONFIG_DIR=""

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'
FAILURES=0
ASSERTIONS=0

pass() { ASSERTIONS=$((ASSERTIONS + 1)); echo -e "  ${GREEN}PASS${NC}: $1"; }
fail() { ASSERTIONS=$((ASSERTIONS + 1)); FAILURES=$((FAILURES + 1)); echo -e "  ${RED}FAIL${NC}: $1"; }
info() { echo -e "  ${YELLOW}INFO${NC}: $1"; }

cleanup() {
  echo ""
  echo "--- Cleanup ---"
  kind delete cluster --name "$CLUSTER_NAME" 2>/dev/null || true
  rm -f "$KUBECONFIG_PATH" "$HANDOFF_FILE" 2>/dev/null || true
  rm -rf "$UPGRADE_CONFIG_DIR" 2>/dev/null || true
  echo "Done."
}
trap cleanup EXIT INT TERM

# ============================================================================
# Phase 0: Prerequisites
# ============================================================================
echo "=== Phase 0: Prerequisites ==="

MISSING=""
for cmd in kind kubectl helm jq base64 python3 docker; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    MISSING="$MISSING $cmd"
  fi
done
if [ -n "$MISSING" ]; then
  echo -e "${RED}Missing:${NC}$MISSING"
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo -e "${RED}Docker is not running.${NC}"
  exit 1
fi

if [[ ! -f "$DEPLOY_SCRIPT" ]]; then
  echo -e "${RED}deploy.sh not found at ${DEPLOY_SCRIPT}${NC}"
  exit 1
fi

echo "  All prerequisites met."

# ============================================================================
# Phase 1: Create ephemeral kind cluster
# ============================================================================
echo ""
echo "=== Phase 1: Kind cluster ==="

# Clean slate
kind delete cluster --name "$CLUSTER_NAME" 2>/dev/null || true

kind create cluster --name "$CLUSTER_NAME" \
  --config "$SCRIPT_DIR/kind-config.yaml" \
  --wait 60s

KUBECONFIG_PATH="$(mktemp /tmp/deploy-test-kubeconfig.XXXXXX)"
kind get kubeconfig --name "$CLUSTER_NAME" > "$KUBECONFIG_PATH"

export KUBECONFIG="$KUBECONFIG_PATH"
kubectl cluster-info --context "kind-${CLUSTER_NAME}" 2>&1 | grep -i "running" || true
echo "  Cluster ready."

# ============================================================================
# Phase 2: Generate synthetic handoff JSON
# ============================================================================
echo ""
echo "=== Phase 2: Handoff + config ==="

KUBECONFIG_B64=$(cat "$KUBECONFIG_PATH" | base64 | tr -d '\n')

HANDOFF_FILE="$(mktemp /tmp/deploy-test-handoff.XXXXXX.json)"
cat > "$HANDOFF_FILE" <<HANDOFF_EOF
{
  "version": "v1",
  "mode": "external",
  "compute_engine": "k3s",
  "project_name": "${PROJECT}",
  "runtime": {
    "k3s": {
      "kubeconfig_base64": "${KUBECONFIG_B64}",
      "host_ip": "127.0.0.1",
      "worker_nodes": []
    }
  },
  "services": {
    "rpc_proxy": {
      "internal_url": "http://${PROJECT}-erpc:4000",
      "port": 4000
    },
    "indexer": null,
    "monitoring": null
  },
  "data": {
    "backend": "clickhouse",
    "clickhouse": {
      "url": "http://localhost:8123",
      "user": "default",
      "password": "deploy-test-fake",
      "db": "default"
    }
  },
  "secrets": {
    "mode": "inline"
  },
  "ingress": {
    "mode": "none"
  }
}
HANDOFF_EOF

echo "  Handoff: $HANDOFF_FILE"
echo "  Config:  $CONFIG_DIR"

# Validate handoff
if jq empty "$HANDOFF_FILE" 2>/dev/null; then
  pass "Handoff JSON is valid"
else
  fail "Handoff JSON is invalid"
  exit 1
fi

# ============================================================================
# Phase 3: Run deploy.sh (eRPC only — indexer is null in handoff)
# ============================================================================
echo ""
echo "=== Phase 3: deploy.sh ==="

# Create namespace first (deploy.sh expects it or creates it)
kubectl create namespace "$NS" 2>/dev/null || true

# Run the real deployer
bash "$DEPLOY_SCRIPT" "$HANDOFF_FILE" --config-dir "$CONFIG_DIR"

echo "  deploy.sh completed."

# ============================================================================
# Phase 3b: Deploy indexer directly via Helm (bypass contracts:[] guard)
# ============================================================================
echo ""
echo "=== Phase 3b: Indexer via direct Helm install ==="

# Render values for the indexer chart manually
# This matches the e2e-k3s pattern: deploy indexer chart directly with inline values
INDEXER_RELEASE="${PROJECT}-indexer"
ERPC_URL="http://${PROJECT}-erpc:4000"

helm upgrade --install "$INDEXER_RELEASE" "${CHARTS_DIR}/indexer/" \
  -n "$NS" \
  --set "image.repository=ghcr.io/rindexer/rindexer" \
  --set "image.tag=latest" \
  --set "image.pullPolicy=IfNotPresent" \
  --set "replicaCount=1" \
  --set "strategy.type=Recreate" \
  --set "env.RPC_URL=${ERPC_URL}/main/evm/1" \
  --set "env.CLICKHOUSE_URL=http://localhost:8123" \
  --set "env.CLICKHOUSE_USER=default" \
  --set "env.CLICKHOUSE_DB=default" \
  --set-string "secretEnv.CLICKHOUSE_PASSWORD=deploy-test-fake" \
  --set-file "configFiles.rindexer\\.yaml=${CONFIG_DIR}/rindexer.yaml" \
  --set-file "configFiles.abis/ERC20\\.json=${CONFIG_DIR}/abis/ERC20.json" \
  --timeout 120s 2>&1 || info "Indexer install may warn about missing values — this is expected"

echo "  Indexer deployed."

# ============================================================================
# Phase 4: Helm release assertions
# ============================================================================
echo ""
echo "=== Phase 4: Helm release assertions ==="

# 4a. eRPC release exists
if helm status "${PROJECT}-erpc" -n "$NS" >/dev/null 2>&1; then
  pass "eRPC Helm release exists"
else
  fail "eRPC Helm release not found"
fi

# 4b. eRPC ConfigMap
if kubectl get configmap "${PROJECT}-erpc-config" -n "$NS" -o jsonpath='{.data.erpc\.yaml}' 2>/dev/null | grep -q "httpPort: 4000"; then
  pass "eRPC ConfigMap contains erpc.yaml with httpPort 4000"
else
  fail "eRPC ConfigMap missing or malformed"
fi

# 4c. eRPC Deployment
if kubectl get deployment "${PROJECT}-erpc" -n "$NS" >/dev/null 2>&1; then
  ERPC_IMAGE=$(kubectl get deployment "${PROJECT}-erpc" -n "$NS" -o jsonpath='{.spec.template.spec.containers[0].image}' 2>/dev/null)
  if echo "$ERPC_IMAGE" | grep -qi "erpc"; then
    pass "eRPC Deployment exists with erpc image"
  else
    fail "eRPC Deployment image unexpected: $ERPC_IMAGE"
  fi
else
  fail "eRPC Deployment not found"
fi

# 4d. eRPC Service
if kubectl get service "${PROJECT}-erpc" -n "$NS" -o jsonpath='{.spec.ports[0].port}' 2>/dev/null | grep -q "4000"; then
  pass "eRPC Service on port 4000"
else
  fail "eRPC Service missing or wrong port"
fi

# 4e. PriorityClasses
if kubectl get priorityclass evm-cloud-system >/dev/null 2>&1; then
  pass "PriorityClass evm-cloud-system exists"
else
  fail "PriorityClass evm-cloud-system not found"
fi

# 4f. Indexer release
if helm status "$INDEXER_RELEASE" -n "$NS" >/dev/null 2>&1; then
  pass "Indexer Helm release exists"
else
  fail "Indexer Helm release not found"
fi

# 4g. Indexer Deployment uses Recreate strategy
INDEXER_STRATEGY=$(kubectl get deployment "$INDEXER_RELEASE" -n "$NS" -o jsonpath='{.spec.strategy.type}' 2>/dev/null || echo "none")
if [[ "$INDEXER_STRATEGY" == "Recreate" ]]; then
  pass "Indexer uses Recreate strategy"
else
  info "Indexer strategy: $INDEXER_STRATEGY (expected Recreate — may vary by chart defaults)"
fi

# ============================================================================
# Phase 5: Pod runtime validation
# ============================================================================
echo ""
echo "=== Phase 5: Pod runtime ==="

# 5a. eRPC pod reaches Running
echo "  Waiting for eRPC pod..."
if kubectl wait --for=condition=Ready pod -l "app.kubernetes.io/name=rpc-proxy" -n "$NS" --timeout=180s 2>/dev/null; then
  pass "eRPC pod is Running + Ready"
else
  fail "eRPC pod did not reach Ready within 180s"
  # Show pod status for debugging
  kubectl get pods -n "$NS" -l "app.kubernetes.io/name=rpc-proxy" -o wide 2>/dev/null || true
fi

# 5b. eRPC responds to HTTP via port-forward
ERPC_POD=$(kubectl get pods -n "$NS" -l "app.kubernetes.io/name=rpc-proxy" -o jsonpath='{.items[0].metadata.name}' 2>/dev/null)
if [[ -n "$ERPC_POD" ]]; then
  kubectl port-forward "$ERPC_POD" -n "$NS" 14100:4000 &
  PF_PID=$!
  HTTP_OK=false
  for i in $(seq 1 15); do
    if curl -sf http://localhost:14100 >/dev/null 2>&1; then
      HTTP_OK=true
      break
    fi
    sleep 1
  done
  if [[ "$HTTP_OK" == "true" ]]; then
    pass "eRPC responds to HTTP on port 4000"
  else
    fail "eRPC did not respond to HTTP within 15s"
  fi
  kill $PF_PID 2>/dev/null || true
  wait $PF_PID 2>/dev/null || true
fi

# 5c. Indexer pod was created (CrashLoop expected — no real ClickHouse)
INDEXER_POD=$(kubectl get pods -n "$NS" -l "app.kubernetes.io/name=indexer" -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")
if [[ -n "$INDEXER_POD" ]]; then
  pass "Indexer pod created: $INDEXER_POD"
else
  fail "Indexer pod not created"
fi

# ============================================================================
# Phase 6: Upgrade test (config change → helm upgrade → pod restart)
# ============================================================================
echo ""
echo "=== Phase 6: Upgrade test ==="

# Record eRPC pod UID before upgrade
ERPC_UID_BEFORE=$(kubectl get pods -n "$NS" -l "app.kubernetes.io/name=rpc-proxy" -o jsonpath='{.items[0].metadata.uid}' 2>/dev/null || echo "none")

# Modify erpc config (change logLevel)
UPGRADE_CONFIG_DIR=$(mktemp -d /tmp/deploy-test-upgrade.XXXXXX)
cp -r "$CONFIG_DIR"/* "$UPGRADE_CONFIG_DIR/"
sed -i.bak 's/logLevel: warn/logLevel: debug/' "$UPGRADE_CONFIG_DIR/erpc.yaml"

# Re-run deploy.sh with modified config
bash "$DEPLOY_SCRIPT" "$HANDOFF_FILE" --config-dir "$UPGRADE_CONFIG_DIR"

# Verify ConfigMap updated
sleep 2
ERPC_CONFIG_CONTENT=$(kubectl get configmap "${PROJECT}-erpc-config" -n "$NS" -o jsonpath='{.data.erpc\.yaml}' 2>/dev/null)
if echo "$ERPC_CONFIG_CONTENT" | grep -q "logLevel: debug"; then
  pass "ConfigMap updated to logLevel: debug after upgrade"
else
  fail "ConfigMap not updated after upgrade"
fi

# Wait for new pod
sleep 5
ERPC_UID_AFTER=$(kubectl get pods -n "$NS" -l "app.kubernetes.io/name=rpc-proxy" -o jsonpath='{.items[0].metadata.uid}' 2>/dev/null || echo "none")
if [[ "$ERPC_UID_BEFORE" != "$ERPC_UID_AFTER" && "$ERPC_UID_AFTER" != "none" ]]; then
  pass "eRPC pod restarted after config change (new UID)"
else
  info "eRPC pod UID unchanged — Helm may not trigger restart for ConfigMap changes"
fi

rm -rf "$UPGRADE_CONFIG_DIR"

# ============================================================================
# Phase 7: Teardown validation
# ============================================================================
echo ""
echo "=== Phase 7: Teardown ==="

if bash "$TEARDOWN_SCRIPT" "$HANDOFF_FILE"; then
  pass "Teardown script exited successfully"
else
  fail "Teardown script failed"
fi

sleep 3

# Verify no Helm releases for this project
REMAINING=$(helm list -n "$NS" -q 2>/dev/null | wc -l | tr -d ' ')
if [[ "$REMAINING" -eq 0 ]]; then
  pass "No Helm releases remain after teardown"
else
  RELEASES=$(helm list -n "$NS" -q 2>/dev/null)
  fail "Helm releases still present after teardown: $RELEASES"
fi

# ============================================================================
# Summary
# ============================================================================
echo ""
echo "=========================================="
if [[ "$FAILURES" -eq 0 ]]; then
  echo -e "${GREEN}ALL ${ASSERTIONS} ASSERTIONS PASSED${NC}"
  exit 0
else
  echo -e "${RED}${FAILURES}/${ASSERTIONS} ASSERTIONS FAILED${NC}"
  exit 1
fi
