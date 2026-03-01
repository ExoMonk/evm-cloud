#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Local K8s validation for EKS modules using kind
# Creates a throwaway cluster, applies EKS K8s modules, asserts resource shape,
# validates Helm charts, and tears down.
# =============================================================================

CLUSTER_NAME="evm-cloud-test"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
KUBECONFIG_PATH="${SCRIPT_DIR}/.kubeconfig"
CHARTS_DIR="${SCRIPT_DIR}/../../deployers/eks/charts"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'
FAILURES=0

pass() { echo -e "  ${GREEN}PASS${NC}: $1"; }
fail() { echo -e "  ${RED}FAIL${NC}: $1"; FAILURES=$((FAILURES + 1)); }

cleanup() {
  echo ""
  echo "--- Cleanup ---"
  cd "$SCRIPT_DIR"
  terraform destroy -auto-approve \
    -var="kubeconfig_path=${KUBECONFIG_PATH}" 2>/dev/null || true
  kind delete cluster --name "$CLUSTER_NAME" 2>/dev/null || true
  rm -f "$KUBECONFIG_PATH"
}
trap cleanup EXIT INT TERM

# --- Phase 0: Prerequisites ---
echo "=== Phase 0: Prerequisites ==="
MISSING=""
for cmd in kind kubectl helm terraform; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    MISSING="$MISSING $cmd"
  fi
done
if [ -n "$MISSING" ]; then
  echo -e "${RED}Missing required tools:${NC}$MISSING"
  echo "Install them before running this test."
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo -e "${RED}Docker is not running.${NC} Start Docker and retry."
  exit 1
fi

echo "  All prerequisites met."

# --- Phase 1: Clean slate + create kind cluster ---
echo ""
echo "=== Phase 1: kind cluster ==="
kind delete cluster --name "$CLUSTER_NAME" 2>/dev/null || true
cd "$SCRIPT_DIR"
rm -rf .terraform terraform.tfstate* .terraform.lock.hcl

kind create cluster --name "$CLUSTER_NAME" \
  --config "$SCRIPT_DIR/kind-config.yaml" \
  --kubeconfig "$KUBECONFIG_PATH" \
  --wait 60s

export KUBECONFIG="$KUBECONFIG_PATH"
kubectl cluster-info --context "kind-${CLUSTER_NAME}" 2>&1 | head -2

# --- Phase 2: Terraform apply (K8s modules only) ---
echo ""
echo "=== Phase 2: Terraform apply ==="
cd "$SCRIPT_DIR"
terraform init -backend=false -input=false
terraform apply -auto-approve -input=false \
  -var="kubeconfig_path=${KUBECONFIG_PATH}"

# --- Phase 3: kubectl assertions ---
echo ""
echo "=== Phase 3: Assertions ==="

# 1. rpc-proxy ConfigMap
if kubectl get configmap kind-test-erpc-config -o jsonpath='{.data.erpc\.yaml}' 2>/dev/null | grep -q "httpPort: 4000"; then
  pass "rpc-proxy ConfigMap contains erpc.yaml with httpPort 4000"
else
  fail "rpc-proxy ConfigMap missing or malformed"
fi

# 2. rpc-proxy Deployment image
if kubectl get deployment kind-test-erpc -o jsonpath='{.spec.template.spec.containers[0].image}' 2>/dev/null | grep -q "erpc"; then
  pass "rpc-proxy Deployment uses erpc image"
else
  fail "rpc-proxy Deployment missing or wrong image"
fi

# 3. rpc-proxy Deployment replicas
REPLICAS=$(kubectl get deployment kind-test-erpc -o jsonpath='{.spec.replicas}' 2>/dev/null)
if [ "$REPLICAS" = "1" ]; then
  pass "rpc-proxy Deployment has 1 replica"
else
  fail "rpc-proxy Deployment replicas: expected 1, got $REPLICAS"
fi

# 4. rpc-proxy Service port
if kubectl get service kind-test-erpc -o jsonpath='{.spec.ports[0].port}' 2>/dev/null | grep -q "4000"; then
  pass "rpc-proxy Service exposes port 4000"
else
  fail "rpc-proxy Service missing or wrong port"
fi

# 5. rpc-proxy Service type
SVC_TYPE=$(kubectl get service kind-test-erpc -o jsonpath='{.spec.type}' 2>/dev/null)
if [ "$SVC_TYPE" = "ClusterIP" ]; then
  pass "rpc-proxy Service type is ClusterIP"
else
  fail "rpc-proxy Service type: expected ClusterIP, got $SVC_TYPE"
fi

# 6. indexer ConfigMap (config)
if kubectl get configmap kind-test-indexer-config -o jsonpath='{.data.rindexer\.yaml}' 2>/dev/null | grep -q "kind-test-indexer"; then
  pass "indexer ConfigMap contains rindexer.yaml"
else
  fail "indexer ConfigMap missing or malformed"
fi

# 7. indexer ConfigMap (ABIs)
if kubectl get configmap kind-test-indexer-abis -o jsonpath='{.data.ERC20\.json}' 2>/dev/null | grep -q "abi"; then
  pass "indexer ABIs ConfigMap contains ERC20.json"
else
  fail "indexer ABIs ConfigMap missing or malformed"
fi

# 8. indexer Secret
if kubectl get secret kind-test-indexer-secrets -o jsonpath='{.data.CLICKHOUSE_PASSWORD}' 2>/dev/null | base64 -d 2>/dev/null | grep -q "test-password"; then
  pass "indexer Secret contains CLICKHOUSE_PASSWORD"
else
  fail "indexer Secret missing or wrong value"
fi

# 9. indexer Deployment strategy
STRATEGY=$(kubectl get deployment kind-test-indexer -o jsonpath='{.spec.strategy.type}' 2>/dev/null)
if [ "$STRATEGY" = "Recreate" ]; then
  pass "indexer Deployment uses Recreate strategy (single-writer)"
else
  fail "indexer Deployment strategy: expected Recreate, got $STRATEGY"
fi

# 10. indexer Deployment replicas
REPLICAS=$(kubectl get deployment kind-test-indexer -o jsonpath='{.spec.replicas}' 2>/dev/null)
if [ "$REPLICAS" = "1" ]; then
  pass "indexer Deployment has 1 replica"
else
  fail "indexer Deployment replicas: expected 1, got $REPLICAS"
fi

# 11. indexer env vars
CONTAINER_ENV=$(kubectl get deployment kind-test-indexer -o jsonpath='{.spec.template.spec.containers[0].env[*].name}' 2>/dev/null)
for expected_var in RPC_URL CLICKHOUSE_URL CLICKHOUSE_USER CLICKHOUSE_DB CLICKHOUSE_PASSWORD; do
  if echo "$CONTAINER_ENV" | grep -q "$expected_var"; then
    pass "indexer Deployment has env var $expected_var"
  else
    fail "indexer Deployment missing env var $expected_var"
  fi
done

# 12. indexer volume mounts (config + abis)
VOLUME_NAMES=$(kubectl get deployment kind-test-indexer -o jsonpath='{.spec.template.spec.volumes[*].name}' 2>/dev/null)
for expected_vol in config abis; do
  if echo "$VOLUME_NAMES" | grep -q "$expected_vol"; then
    pass "indexer Deployment has volume $expected_vol"
  else
    fail "indexer Deployment missing volume $expected_vol"
  fi
done

# --- Phase 4: Runtime validation ---
echo ""
echo "=== Phase 4: Runtime validation ==="

# Helper: wait for a pod to have a container status (image pulled + attempted start)
# Usage: wait_for_pod_container <label-selector> <timeout-seconds>
# Sets: POD_NAME, POD_PHASE
wait_for_pod_container() {
  local selector="$1" timeout="$2"
  POD_NAME="" POD_PHASE=""
  local elapsed=0
  while [ $elapsed -lt "$timeout" ]; do
    POD_NAME=$(kubectl get pods -l "$selector" -o jsonpath='{.items[0].metadata.name}' 2>/dev/null)
    if [ -n "$POD_NAME" ]; then
      local state
      state=$(kubectl get pod "$POD_NAME" -o jsonpath='{.status.containerStatuses[0].state}' 2>/dev/null || echo "")
      if [ -n "$state" ]; then
        POD_PHASE=$(kubectl get pod "$POD_NAME" -o jsonpath='{.status.phase}' 2>/dev/null)
        return 0
      fi
    fi
    sleep 5
    elapsed=$((elapsed + 5))
  done
  return 1
}

# --- eRPC: should fully start (projects: [] is valid, HTTP server binds on :4000) ---
echo "  Waiting for eRPC deployment to be available (image pull + start, up to 180s)..."
if kubectl wait --for=condition=available deployment/kind-test-erpc --timeout=180s 2>/dev/null; then
  ERPC_POD=$(kubectl get pods -l app=kind-test-erpc -o jsonpath='{.items[0].metadata.name}' 2>/dev/null)
  POD_NAME="$ERPC_POD"
  POD_PHASE=$(kubectl get pod "$POD_NAME" -o jsonpath='{.status.phase}' 2>/dev/null)
  pass "eRPC deployment available (pod: $POD_PHASE)"

  if [ "$POD_PHASE" = "Running" ]; then
    pass "eRPC pod is Running"

    # Check logs for startup
    ERPC_LOGS=$(kubectl logs "$POD_NAME" --tail=20 2>/dev/null || echo "")
    if [ -n "$ERPC_LOGS" ]; then
      pass "eRPC produced logs"
      echo "    (last log: $(echo "$ERPC_LOGS" | tail -1 | cut -c1-120))"
    else
      fail "eRPC is Running but produced no logs"
    fi

    # Port-forward and verify HTTP
    kubectl port-forward "service/kind-test-erpc" 14000:4000 >/dev/null 2>&1 &
    ERPC_PF_PID=$!
    sleep 3

    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:14000/ 2>/dev/null || echo "000")
    if [ "$HTTP_CODE" != "000" ]; then
      pass "eRPC HTTP responds (status $HTTP_CODE)"
    else
      fail "eRPC did not respond to HTTP request"
    fi

    kill "$ERPC_PF_PID" 2>/dev/null || true
    wait "$ERPC_PF_PID" 2>/dev/null || true
  else
    fail "eRPC pod did not reach Running (stuck in $POD_PHASE)"
  fi
else
  fail "eRPC deployment did not become available within 180s"
fi

# --- rindexer: will crash on ClickHouse connect, but should pull image + attempt start ---
echo "  Waiting for rindexer pod (image pull + start, up to 120s)..."
if wait_for_pod_container "app=kind-test-indexer" 120; then
  pass "rindexer pod created (phase: $POD_PHASE)"

  # Give container a moment to produce logs before checking
  sleep 3

  INDEXER_LOGS=$(kubectl logs "$POD_NAME" --tail=30 2>/dev/null || echo "")
  if [ -n "$INDEXER_LOGS" ]; then
    pass "rindexer produced logs (container ran)"
    echo "    (last log: $(echo "$INDEXER_LOGS" | tail -1 | cut -c1-120))"
  else
    fail "rindexer pod started but produced no logs"
  fi
else
  fail "rindexer pod did not start within 120s"
fi

# --- Phase 5: Helm chart dry-run validation ---
echo ""
echo "=== Phase 5: Helm chart dry-run ==="

if helm template test-rpc "$CHARTS_DIR/rpc-proxy" \
    --values "$CHARTS_DIR/rpc-proxy/values.yaml" 2>&1 \
    | kubectl apply --dry-run=server -f - >/dev/null 2>&1; then
  pass "rpc-proxy Helm chart renders and validates against K8s API"
else
  fail "rpc-proxy Helm chart dry-run failed"
fi

if helm template test-idx "$CHARTS_DIR/indexer" \
    --values "$CHARTS_DIR/indexer/values.yaml" 2>&1 \
    | kubectl apply --dry-run=server -f - >/dev/null 2>&1; then
  pass "indexer Helm chart renders and validates against K8s API"
else
  fail "indexer Helm chart dry-run failed"
fi

# --- Summary ---
echo ""
echo "=== Results ==="
if [ "$FAILURES" -eq 0 ]; then
  echo -e "${GREEN}All assertions passed.${NC}"
  exit 0
else
  echo -e "${RED}${FAILURES} assertion(s) failed.${NC}"
  exit 1
fi
