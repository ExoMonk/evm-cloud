#!/usr/bin/env bash
# =============================================================================
# E2E k3s validation — deploys workloads on a persistent k3s VPS via the real
# deployer pipeline, validates pods, networking, and k3s-specific behavior.
#
# Requires:
#   E2E_KUBECONFIG — path to kubeconfig for the persistent k3s cluster
#
# Usage:
#   E2E_KUBECONFIG=~/.kube/evm-cloud-e2e make test-e2e-k8s
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'
FAILURES=0
PASS_COUNT=0

pass() { echo -e "  ${GREEN}PASS${NC}: $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "  ${RED}FAIL${NC}: $1"; FAILURES=$((FAILURES + 1)); }
info() { echo -e "  ${YELLOW}INFO${NC}: $1"; }

# --- Configuration ---

E2E_TIMEOUT="${E2E_TIMEOUT:-300}"
RUN_ID="${GITHUB_RUN_ID:-$(date +%s)}"
E2E_NS="e2e-${RUN_ID}"
PROJECT_NAME="e2e-${RUN_ID}"

KUBECONFIG_FILE="${E2E_KUBECONFIG:-${KUBECONFIG:-}}"
TEST_DIR=$(mktemp -d /tmp/e2e-k3s.XXXXXX)

# --- Cleanup on exit ---

cleanup() {
  echo ""
  echo "--- Cleanup ---"

  # Delete test namespace (all resources with it)
  kubectl delete namespace "$E2E_NS" --timeout=60s 2>/dev/null || true

  # Clean stale test namespaces (>30 min old)
  for ns in $(kubectl get ns -o jsonpath='{.items[*].metadata.name}' 2>/dev/null); do
    case "$ns" in e2e-*)
      CREATED=$(kubectl get ns "$ns" -o jsonpath='{.metadata.creationTimestamp}' 2>/dev/null || echo "")
      if [ -n "$CREATED" ]; then
        # macOS + GNU compatible age calculation
        NOW=$(date +%s)
        CREATED_TS=$(date -jf "%Y-%m-%dT%H:%M:%SZ" "$CREATED" +%s 2>/dev/null || date -d "$CREATED" +%s 2>/dev/null || echo "$NOW")
        AGE_SEC=$(( NOW - CREATED_TS ))
        if [ "$AGE_SEC" -gt 1800 ]; then
          info "Cleaning stale namespace: $ns (age: ${AGE_SEC}s)"
          kubectl delete ns "$ns" --timeout=60s 2>/dev/null || true
        fi
      fi
      ;;
    esac
  done

  # Kill any port-forward processes
  kill "$PF_PID" 2>/dev/null || true

  rm -rf "$TEST_DIR"
}
trap cleanup EXIT INT TERM

PF_PID=""

# --- Helper: wait for a pod to have a container status ---
# Sets: POD_NAME, POD_PHASE
wait_for_pod_container() {
  local selector="$1" namespace="$2" timeout="$3"
  POD_NAME="" POD_PHASE=""
  local elapsed=0
  while [ $elapsed -lt "$timeout" ]; do
    POD_NAME=$(kubectl get pods -l "$selector" -n "$namespace" -o jsonpath='{.items[0].metadata.name}' 2>/dev/null)
    if [ -n "$POD_NAME" ]; then
      local state
      state=$(kubectl get pod "$POD_NAME" -n "$namespace" -o jsonpath='{.status.containerStatuses[0].state}' 2>/dev/null || echo "")
      if [ -n "$state" ]; then
        POD_PHASE=$(kubectl get pod "$POD_NAME" -n "$namespace" -o jsonpath='{.status.phase}' 2>/dev/null)
        return 0
      fi
    fi
    sleep 5
    elapsed=$((elapsed + 5))
  done
  return 1
}

# =============================================================================
# Phase 0: Prerequisites
# =============================================================================
echo "=== Phase 0: Prerequisites ==="

MISSING=""
for cmd in kubectl helm jq curl; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    MISSING="$MISSING $cmd"
  fi
done
if [ -n "$MISSING" ]; then
  echo -e "${RED}Missing required tools:${NC}$MISSING"
  exit 1
fi

if [ -z "$KUBECONFIG_FILE" ] || [ ! -f "$KUBECONFIG_FILE" ]; then
  echo -e "${RED}E2E_KUBECONFIG or KUBECONFIG must point to a valid kubeconfig file.${NC}"
  echo "  Current value: '${KUBECONFIG_FILE:-<not set>}'"
  echo ""
  echo "  Setup:"
  echo "    cd tests/e2e-k3s/infra && terraform apply -var-file=e2e.tfvars"
  echo "    terraform output -json workload_handoff | jq -r '.runtime.k3s.kubeconfig_base64' | base64 -d > ~/.kube/evm-cloud-e2e"
  echo "    export E2E_KUBECONFIG=~/.kube/evm-cloud-e2e"
  exit 1
fi
export KUBECONFIG="$KUBECONFIG_FILE"

if [ ! -x "$REPO_ROOT/deployers/k3s/deploy.sh" ]; then
  echo -e "${RED}deployers/k3s/deploy.sh not found or not executable${NC}"
  exit 1
fi

echo "  All prerequisites met."
echo "  Namespace: $E2E_NS"
echo "  Project: $PROJECT_NAME"

# =============================================================================
# Phase 1: Cluster Health
# =============================================================================
echo ""
echo "=== Phase 1: Cluster health ==="

# Node is Ready
NODE_STATUS=$(kubectl get nodes -o jsonpath='{.items[0].status.conditions[?(@.type=="Ready")].status}' 2>/dev/null || echo "")
if [ "$NODE_STATUS" = "True" ]; then
  pass "k3s node is Ready"
else
  fail "k3s node is not Ready (status: '$NODE_STATUS')"
  echo -e "${RED}Cluster appears unhealthy. Aborting E2E test.${NC}"
  exit 1
fi

# CoreDNS is running
COREDNS_READY=$(kubectl get deploy coredns -n kube-system -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
if [ "${COREDNS_READY:-0}" -ge 1 ]; then
  pass "CoreDNS is running (${COREDNS_READY} replicas)"
else
  fail "CoreDNS not ready"
fi

# System pods healthy
UNHEALTHY_PODS=$(kubectl get pods -n kube-system --field-selector=status.phase!=Running,status.phase!=Succeeded -o name 2>/dev/null | wc -l | tr -d ' ')
if [ "$UNHEALTHY_PODS" -eq 0 ]; then
  pass "All kube-system pods healthy"
else
  fail "$UNHEALTHY_PODS unhealthy pod(s) in kube-system"
fi

# k3s version
K3S_VERSION=$(kubectl get nodes -o jsonpath='{.items[0].status.nodeInfo.kubeletVersion}' 2>/dev/null || echo "unknown")
pass "k3s version: $K3S_VERSION"

# =============================================================================
# Phase 2: Deploy via real deployer
# =============================================================================
echo ""
echo "=== Phase 2: Deploy workloads ==="

# Create test namespace
kubectl create namespace "$E2E_NS" 2>/dev/null || true

# Clean any stale releases from previous failed runs
for rel in $(helm list -n "$E2E_NS" -q 2>/dev/null); do
  info "Cleaning stale release: $rel"
  helm uninstall "$rel" -n "$E2E_NS" --wait 2>/dev/null || true
done

# Clean ALL e2e-* releases in default namespace (from previous runs)
for rel in $(helm list -q 2>/dev/null | grep "^e2e-" || true); do
  info "Cleaning stale release in default ns: $rel"
  helm uninstall "$rel" --wait 2>/dev/null || true
done

# Build synthetic handoff JSON
KUBECONFIG_B64=$(base64 < "$KUBECONFIG_FILE" | tr -d '\n')
CLUSTER_ENDPOINT=$(kubectl config view --minify -o jsonpath='{.clusters[0].cluster.server}' 2>/dev/null || echo "https://localhost:6443")

cat > "$TEST_DIR/handoff.json" <<EOF
{
  "version": "v1",
  "mode": "external",
  "compute_engine": "k3s",
  "project_name": "$PROJECT_NAME",
  "runtime": {
    "k3s": {
      "kubeconfig_base64": "$KUBECONFIG_B64",
      "cluster_endpoint": "$CLUSTER_ENDPOINT"
    }
  },
  "services": {
    "rpc_proxy": { "port": 4000, "enabled": true },
    "indexer": null
  },
  "data": {
    "backend": "clickhouse",
    "clickhouse": {
      "url": "http://localhost:8123",
      "user": "default",
      "password": "e2e-test-not-real",
      "db": "default"
    }
  }
}
EOF
chmod 0600 "$TEST_DIR/handoff.json"

# Run the actual deployer (eRPC only — indexer is null in handoff to avoid --wait hang)
echo "  Running deployers/k3s/deploy.sh (eRPC)..."
if "$REPO_ROOT/deployers/k3s/deploy.sh" "$TEST_DIR/handoff.json" \
    --config-dir "$SCRIPT_DIR/config" 2>&1; then
  pass "deployers/k3s/deploy.sh completed (eRPC)"
else
  fail "deployers/k3s/deploy.sh failed"
  echo -e "${RED}Deployer failed. Skipping remaining phases.${NC}"
  exit 1
fi

# Deploy indexer separately WITHOUT --wait (rindexer CrashLoops on fake ClickHouse — by design)
echo "  Deploying indexer chart directly (no --wait — CrashLoop expected)..."

# Render indexer values using the deployer's own scripts
INDEXER_HANDOFF_FILE=$(mktemp /tmp/e2e-indexer-handoff.XXXXXX)
jq '.services.indexer = {"enabled": true}' "$TEST_DIR/handoff.json" > "$INDEXER_HANDOFF_FILE"
INDEXER_VALUES_DIR=$(mktemp -d /tmp/e2e-indexer-values.XXXXXX)
"${REPO_ROOT}/deployers/k3s/scripts/render-values.sh" "$INDEXER_HANDOFF_FILE" "$INDEXER_VALUES_DIR"
"${REPO_ROOT}/deployers/eks/scripts/populate-values-from-config-bundle.sh" \
  --values-dir "$INDEXER_VALUES_DIR" --config-dir "$SCRIPT_DIR/config"

helm upgrade --install "${PROJECT_NAME}-indexer" "${REPO_ROOT}/deployers/charts/indexer/" \
  -f "${INDEXER_VALUES_DIR}/indexer-values.yaml" --timeout 5m --create-namespace 2>&1
if [ $? -eq 0 ]; then
  pass "indexer Helm release installed (no --wait)"
else
  fail "indexer Helm release install failed"
fi
rm -rf "$INDEXER_HANDOFF_FILE" "$INDEXER_VALUES_DIR"

# =============================================================================
# Phase 3: Resource assertions
# =============================================================================
echo ""
echo "=== Phase 3: Resource assertions ==="

# Note: deploy.sh deploys to default namespace (no -n flag in the deployer).
# All resource checks use default namespace.
NS="default"

# 3.1 rpc-proxy ConfigMap
if kubectl get configmap "${PROJECT_NAME}-erpc-config" -n "$NS" -o jsonpath='{.data.erpc\.yaml}' 2>/dev/null | grep -q "httpPort: 4000"; then
  pass "rpc-proxy ConfigMap contains erpc.yaml with httpPort 4000"
else
  fail "rpc-proxy ConfigMap missing or malformed"
fi

# 3.2 rpc-proxy Deployment image
if kubectl get deployment "${PROJECT_NAME}-erpc" -n "$NS" -o jsonpath='{.spec.template.spec.containers[0].image}' 2>/dev/null | grep -q "erpc"; then
  pass "rpc-proxy Deployment uses erpc image"
else
  fail "rpc-proxy Deployment missing or wrong image"
fi

# 3.3 rpc-proxy Deployment replicas
REPLICAS=$(kubectl get deployment "${PROJECT_NAME}-erpc" -n "$NS" -o jsonpath='{.spec.replicas}' 2>/dev/null)
if [ "$REPLICAS" = "1" ]; then
  pass "rpc-proxy Deployment has 1 replica"
else
  fail "rpc-proxy Deployment replicas: expected 1, got $REPLICAS"
fi

# 3.4 rpc-proxy Service port
if kubectl get service "${PROJECT_NAME}-erpc" -n "$NS" -o jsonpath='{.spec.ports[0].port}' 2>/dev/null | grep -q "4000"; then
  pass "rpc-proxy Service exposes port 4000"
else
  fail "rpc-proxy Service missing or wrong port"
fi

# 3.5 rpc-proxy Service type
SVC_TYPE=$(kubectl get service "${PROJECT_NAME}-erpc" -n "$NS" -o jsonpath='{.spec.type}' 2>/dev/null)
if [ "$SVC_TYPE" = "ClusterIP" ]; then
  pass "rpc-proxy Service type is ClusterIP"
else
  fail "rpc-proxy Service type: expected ClusterIP, got $SVC_TYPE"
fi

# 3.6 indexer ConfigMap (config)
if kubectl get configmap "${PROJECT_NAME}-indexer-config" -n "$NS" -o jsonpath='{.data.rindexer\.yaml}' 2>/dev/null | grep -q "evm-cloud-e2e"; then
  pass "indexer ConfigMap contains rindexer.yaml"
else
  fail "indexer ConfigMap missing or malformed"
fi

# 3.7 indexer ConfigMap (ABIs)
if kubectl get configmap "${PROJECT_NAME}-indexer-abis" -n "$NS" -o jsonpath='{.data.ERC20\.json}' 2>/dev/null | grep -q "Transfer"; then
  pass "indexer ABIs ConfigMap contains ERC20.json"
else
  fail "indexer ABIs ConfigMap missing or malformed"
fi

# 3.8 indexer Secret
if kubectl get secret "${PROJECT_NAME}-indexer-secrets" -n "$NS" -o jsonpath='{.data.CLICKHOUSE_PASSWORD}' 2>/dev/null | base64 -d 2>/dev/null | grep -q "e2e-test-not-real"; then
  pass "indexer Secret contains CLICKHOUSE_PASSWORD"
else
  fail "indexer Secret missing or wrong value"
fi

# 3.9 indexer Deployment strategy
STRATEGY=$(kubectl get deployment "${PROJECT_NAME}-indexer" -n "$NS" -o jsonpath='{.spec.strategy.type}' 2>/dev/null)
if [ "$STRATEGY" = "Recreate" ]; then
  pass "indexer Deployment uses Recreate strategy"
else
  fail "indexer Deployment strategy: expected Recreate, got $STRATEGY"
fi

# 3.10 indexer Deployment replicas
REPLICAS=$(kubectl get deployment "${PROJECT_NAME}-indexer" -n "$NS" -o jsonpath='{.spec.replicas}' 2>/dev/null)
if [ "$REPLICAS" = "1" ]; then
  pass "indexer Deployment has 1 replica"
else
  fail "indexer Deployment replicas: expected 1, got $REPLICAS"
fi

# 3.11 indexer env vars
CONTAINER_ENV=$(kubectl get deployment "${PROJECT_NAME}-indexer" -n "$NS" -o jsonpath='{.spec.template.spec.containers[0].env[*].name}' 2>/dev/null)
for expected_var in RPC_URL CLICKHOUSE_URL CLICKHOUSE_USER CLICKHOUSE_DB CLICKHOUSE_PASSWORD; do
  if echo "$CONTAINER_ENV" | grep -q "$expected_var"; then
    pass "indexer Deployment has env var $expected_var"
  else
    fail "indexer Deployment missing env var $expected_var"
  fi
done

# 3.12 indexer volume mounts
VOLUME_NAMES=$(kubectl get deployment "${PROJECT_NAME}-indexer" -n "$NS" -o jsonpath='{.spec.template.spec.volumes[*].name}' 2>/dev/null)
for expected_vol in config abis; do
  if echo "$VOLUME_NAMES" | grep -q "$expected_vol"; then
    pass "indexer Deployment has volume $expected_vol"
  else
    fail "indexer Deployment missing volume $expected_vol"
  fi
done

# =============================================================================
# Phase 4: Runtime validation
# =============================================================================
echo ""
echo "=== Phase 4: Runtime validation ==="

# --- eRPC: should fully start ---
echo "  Waiting for eRPC deployment (up to 180s)..."
if kubectl wait --for=condition=available "deployment/${PROJECT_NAME}-erpc" -n "$NS" --timeout=180s 2>/dev/null; then
  ERPC_POD=$(kubectl get pods -l "app.kubernetes.io/instance=${PROJECT_NAME}-erpc" -n "$NS" -o jsonpath='{.items[0].metadata.name}' 2>/dev/null)
  POD_PHASE=$(kubectl get pod "$ERPC_POD" -n "$NS" -o jsonpath='{.status.phase}' 2>/dev/null)
  pass "eRPC deployment available (pod: $POD_PHASE)"

  if [ "$POD_PHASE" = "Running" ]; then
    pass "eRPC pod is Running"

    ERPC_LOGS=$(kubectl logs "$ERPC_POD" -n "$NS" --tail=20 2>/dev/null || echo "")
    if [ -n "$ERPC_LOGS" ]; then
      pass "eRPC produced logs"
      echo "    (last log: $(echo "$ERPC_LOGS" | tail -1 | cut -c1-120))"
    else
      fail "eRPC is Running but produced no logs"
    fi

    # Port-forward and verify HTTP
    kubectl port-forward "svc/${PROJECT_NAME}-erpc" 14000:4000 -n "$NS" >/dev/null 2>&1 &
    PF_PID=$!
    sleep 3

    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 5 http://localhost:14000/ 2>/dev/null || echo "000")
    if [ "$HTTP_CODE" != "000" ]; then
      pass "eRPC HTTP responds via port-forward (status $HTTP_CODE)"
    else
      fail "eRPC did not respond to HTTP request"
    fi

    kill "$PF_PID" 2>/dev/null || true
    wait "$PF_PID" 2>/dev/null || true
    PF_PID=""
  else
    fail "eRPC pod did not reach Running (stuck in $POD_PHASE)"
  fi
else
  fail "eRPC deployment did not become available within 180s"
fi

# --- rindexer: will CrashLoop, but should pull image + attempt start ---
echo "  Waiting for rindexer pod (up to 120s)..."
if wait_for_pod_container "app.kubernetes.io/instance=${PROJECT_NAME}-indexer" "$NS" 120; then
  pass "rindexer pod created (phase: $POD_PHASE)"

  sleep 3
  INDEXER_LOGS=$(kubectl logs "$POD_NAME" -n "$NS" --tail=30 2>/dev/null || echo "")
  if [ -n "$INDEXER_LOGS" ]; then
    pass "rindexer produced logs (container ran)"
    echo "    (last log: $(echo "$INDEXER_LOGS" | tail -1 | cut -c1-120))"
  else
    fail "rindexer pod started but produced no logs"
  fi
else
  fail "rindexer pod did not start within 120s"
fi

# =============================================================================
# Phase 4.5: Upgrade/redeploy test (zero-downtime config change)
# =============================================================================
echo ""
echo "=== Phase 4.5: Upgrade test ==="

# Record pre-upgrade state (use UID as reliable identity — pod name can be reused)
PRE_UPGRADE_POD=$(kubectl get pods -l "app.kubernetes.io/instance=${PROJECT_NAME}-erpc" -n "$NS" -o jsonpath='{.items[0].metadata.name}' 2>/dev/null)
PRE_UPGRADE_UID=$(kubectl get pod "$PRE_UPGRADE_POD" -n "$NS" -o jsonpath='{.metadata.uid}' 2>/dev/null || echo "")
# jsonpath doesn't handle / in annotation keys well — use go-template
PRE_UPGRADE_CHECKSUM=$(kubectl get pod "$PRE_UPGRADE_POD" -n "$NS" -o go-template='{{index .metadata.annotations "checksum/config"}}' 2>/dev/null || echo "")
info "Pre-upgrade pod: $PRE_UPGRADE_POD (uid: ${PRE_UPGRADE_UID:0:8}, checksum: ${PRE_UPGRADE_CHECKSUM:0:12}...)"

# Start a background uptime monitor — polls eRPC HTTP via port-forward during upgrade
kubectl port-forward "svc/${PROJECT_NAME}-erpc" 14001:4000 -n "$NS" >/dev/null 2>&1 &
UPGRADE_PF_PID=$!
sleep 2

UPTIME_LOG="$TEST_DIR/uptime.log"
(
  while true; do
    CODE=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 2 http://localhost:14001/ 2>/dev/null || echo "000")
    echo "$(date +%s) $CODE" >> "$UPTIME_LOG"
    sleep 1
  done
) &
UPTIME_PID=$!

# Modify eRPC config and re-deploy
mkdir -p "$TEST_DIR/config-modified/abis"
sed 's/logLevel: warn/logLevel: debug/' "$SCRIPT_DIR/config/erpc.yaml" > "$TEST_DIR/config-modified/erpc.yaml"
cp "$SCRIPT_DIR/config/rindexer.yaml" "$TEST_DIR/config-modified/rindexer.yaml"
cp "$SCRIPT_DIR/config/abis/ERC20.json" "$TEST_DIR/config-modified/abis/ERC20.json"

echo "  Re-deploying with modified config (logLevel: warn → debug)..."
if "$REPO_ROOT/deployers/k3s/deploy.sh" "$TEST_DIR/handoff.json" \
    --config-dir "$TEST_DIR/config-modified" >/dev/null 2>&1; then
  pass "Helm upgrade completed"
else
  fail "Helm upgrade failed"
fi

# Wait for rollout to complete
if kubectl rollout status "deployment/${PROJECT_NAME}-erpc" -n "$NS" --timeout=120s 2>/dev/null; then
  pass "eRPC rollout completed after upgrade"
else
  fail "eRPC rollout did not complete after upgrade"
fi

# Stop uptime monitor
kill "$UPTIME_PID" 2>/dev/null || true
wait "$UPTIME_PID" 2>/dev/null || true
kill "$UPGRADE_PF_PID" 2>/dev/null || true
wait "$UPGRADE_PF_PID" 2>/dev/null || true

# Analyze uptime during upgrade
if [ -f "$UPTIME_LOG" ]; then
  TOTAL_CHECKS=$(wc -l < "$UPTIME_LOG" | tr -d ' ')
  FAILED_CHECKS=$(grep -c " 000$" "$UPTIME_LOG" || true)
  if [ "$TOTAL_CHECKS" -gt 0 ] && [ "$FAILED_CHECKS" -eq 0 ]; then
    pass "Zero-downtime upgrade ($TOTAL_CHECKS health checks, 0 failures)"
  elif [ "$TOTAL_CHECKS" -gt 0 ]; then
    fail "Downtime detected during upgrade ($FAILED_CHECKS/$TOTAL_CHECKS checks failed)"
  else
    info "No uptime data collected (upgrade was very fast)"
  fi
fi

# Verify ConfigMap has new config
UPDATED_CONFIG=$(kubectl get configmap "${PROJECT_NAME}-erpc-config" -n "$NS" -o jsonpath='{.data.erpc\.yaml}' 2>/dev/null)
if echo "$UPDATED_CONFIG" | grep -q "logLevel: debug"; then
  pass "ConfigMap updated to logLevel: debug"
else
  fail "ConfigMap not updated after upgrade"
fi

# Verify the pod actually restarted with new config
POST_UPGRADE_POD=$(kubectl get pods -l "app.kubernetes.io/instance=${PROJECT_NAME}-erpc" -n "$NS" -o jsonpath='{.items[0].metadata.name}' 2>/dev/null)
POST_UPGRADE_UID=$(kubectl get pod "$POST_UPGRADE_POD" -n "$NS" -o jsonpath='{.metadata.uid}' 2>/dev/null || echo "")
POST_UPGRADE_CHECKSUM=$(kubectl get pod "$POST_UPGRADE_POD" -n "$NS" -o go-template='{{index .metadata.annotations "checksum/config"}}' 2>/dev/null || echo "")
info "Post-upgrade pod: $POST_UPGRADE_POD (uid: ${POST_UPGRADE_UID:0:8}, checksum: ${POST_UPGRADE_CHECKSUM:0:12}...)"

if [ "$PRE_UPGRADE_UID" != "$POST_UPGRADE_UID" ]; then
  pass "Pod recreated after config change (new uid: ${POST_UPGRADE_UID:0:8})"
  if [ "$PRE_UPGRADE_CHECKSUM" != "$POST_UPGRADE_CHECKSUM" ]; then
    pass "Config checksum changed (${PRE_UPGRADE_CHECKSUM:0:12} → ${POST_UPGRADE_CHECKSUM:0:12})"
  else
    info "Checksum unchanged but pod was recreated — config embedded differently"
  fi
else
  fail "Pod did not restart after config change (same uid: $PRE_UPGRADE_UID)"
fi

# Verify new pod is Running and HTTP responds with new config
POST_PHASE=$(kubectl get pod "$POST_UPGRADE_POD" -n "$NS" -o jsonpath='{.status.phase}' 2>/dev/null)
if [ "$POST_PHASE" = "Running" ]; then
  pass "Upgraded eRPC pod is Running"
else
  fail "Upgraded eRPC pod not Running (phase: $POST_PHASE)"
fi

# Quick HTTP check on the upgraded pod
kubectl port-forward "svc/${PROJECT_NAME}-erpc" 14002:4000 -n "$NS" >/dev/null 2>&1 &
PF_PID=$!
sleep 3
POST_HTTP=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 5 http://localhost:14002/ 2>/dev/null || echo "000")
kill "$PF_PID" 2>/dev/null || true
wait "$PF_PID" 2>/dev/null || true
PF_PID=""

if [ "$POST_HTTP" != "000" ]; then
  pass "Upgraded eRPC responds to HTTP (status $POST_HTTP)"
else
  fail "Upgraded eRPC not responding to HTTP"
fi

# =============================================================================
# Phase 5: Networking validation
# =============================================================================
echo ""
echo "=== Phase 5: Networking ==="

# Patch service to NodePort for in-cluster testing
kubectl patch svc "${PROJECT_NAME}-erpc" -n "$NS" \
  -p '{"spec":{"type":"NodePort"}}' 2>/dev/null || true
sleep 2

NODEPORT=$(kubectl get svc "${PROJECT_NAME}-erpc" -n "$NS" -o jsonpath='{.spec.ports[0].nodePort}' 2>/dev/null || echo "")
HOST_IP=$(kubectl get nodes -o jsonpath='{.items[0].status.addresses[?(@.type=="InternalIP")].address}' 2>/dev/null || echo "")

if [ -n "$NODEPORT" ] && [ -n "$HOST_IP" ]; then
  # Test NodePort from within the cluster
  NODEPORT_RESULT=$(kubectl run nodeport-test-$RUN_ID --rm -i --restart=Never \
    --image=curlimages/curl:8.5.0 -n "$NS" --timeout=30s \
    -- curl -s -o /dev/null -w "%{http_code}" --connect-timeout 5 "http://${HOST_IP}:${NODEPORT}/" 2>/dev/null || echo "000")
  if [ "$NODEPORT_RESULT" != "000" ]; then
    pass "eRPC accessible via NodePort from within cluster (status $NODEPORT_RESULT)"
  else
    fail "eRPC not accessible via NodePort from within cluster"
  fi
else
  fail "Could not determine NodePort or host IP for networking test"
fi

# DNS resolution via Job (more reliable than kubectl run --rm -i)
cat > "$TEST_DIR/dns-job.yaml" <<'DNSJOB'
apiVersion: batch/v1
kind: Job
metadata:
  name: dns-test
spec:
  backoffLimit: 0
  activeDeadlineSeconds: 30
  template:
    spec:
      containers:
      - name: dns
        image: curlimages/curl:8.5.0
        command: ["sh", "-c", "nslookup ethereum-rpc.publicnode.com"]
      restartPolicy: Never
DNSJOB

kubectl apply -f "$TEST_DIR/dns-job.yaml" -n "$NS" 2>/dev/null
if kubectl wait --for=condition=Complete "job/dns-test" -n "$NS" --timeout=45s 2>/dev/null; then
  pass "CoreDNS resolves external hostnames (ethereum-rpc.publicnode.com)"
else
  fail "DNS resolution job failed or timed out"
fi
kubectl delete job dns-test -n "$NS" 2>/dev/null || true

# =============================================================================
# Phase 6: k3s-specific validation
# =============================================================================
echo ""
echo "=== Phase 6: k3s-specific ==="

# local-path PVC with pod mount
cat > "$TEST_DIR/pvc-test.yaml" <<'PVCTEST'
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: e2e-test-pvc
spec:
  accessModes: [ReadWriteOnce]
  storageClassName: local-path
  resources:
    requests:
      storage: 64Mi
---
apiVersion: v1
kind: Pod
metadata:
  name: e2e-pvc-test
spec:
  containers:
  - name: test
    image: busybox:1.36
    command: ["sh", "-c", "echo ok > /data/test && sleep 30"]
    volumeMounts:
    - name: data
      mountPath: /data
  volumes:
  - name: data
    persistentVolumeClaim:
      claimName: e2e-test-pvc
  restartPolicy: Never
PVCTEST

kubectl apply -f "$TEST_DIR/pvc-test.yaml" -n "$NS" 2>/dev/null
if kubectl wait --for=condition=Ready "pod/e2e-pvc-test" -n "$NS" --timeout=60s 2>/dev/null; then
  PVC_STATUS=$(kubectl get pvc e2e-test-pvc -n "$NS" -o jsonpath='{.status.phase}' 2>/dev/null)
  if [ "$PVC_STATUS" = "Bound" ]; then
    pass "local-path PVC bound after pod mount"
  else
    fail "local-path PVC status: $PVC_STATUS (expected Bound)"
  fi
else
  fail "PVC test pod did not become Ready within 60s"
fi
kubectl delete pod e2e-pvc-test -n "$NS" 2>/dev/null || true
kubectl delete pvc e2e-test-pvc -n "$NS" 2>/dev/null || true

# Pod restart resilience
echo "  Testing pod restart resilience..."
ERPC_POD_BEFORE=$(kubectl get pods -l "app.kubernetes.io/instance=${PROJECT_NAME}-erpc" -n "$NS" -o jsonpath='{.items[0].metadata.name}' 2>/dev/null)
kubectl delete pod "$ERPC_POD_BEFORE" -n "$NS" 2>/dev/null || true
if kubectl wait --for=condition=available "deployment/${PROJECT_NAME}-erpc" -n "$NS" --timeout=120s 2>/dev/null; then
  ERPC_POD_AFTER=$(kubectl get pods -l "app.kubernetes.io/instance=${PROJECT_NAME}-erpc" -n "$NS" -o jsonpath='{.items[0].metadata.name}' 2>/dev/null)
  if [ "$ERPC_POD_BEFORE" != "$ERPC_POD_AFTER" ]; then
    pass "eRPC pod recovered after delete (new pod: $ERPC_POD_AFTER)"
  else
    fail "eRPC pod name unchanged after delete — pod may not have restarted"
  fi
else
  fail "eRPC deployment did not recover within 120s after pod delete"
fi

# =============================================================================
# Phase 7: Teardown
# =============================================================================
echo ""
echo "=== Phase 7: Teardown ==="

# Run the actual teardown script
if "$REPO_ROOT/deployers/k3s/teardown.sh" "$TEST_DIR/handoff.json" 2>&1; then
  pass "teardown.sh completed"
else
  fail "teardown.sh failed"
fi

# Clean up PVC test resources and NodePort test pods
kubectl delete pod -l run=nodeport-test-$RUN_ID -n "$NS" 2>/dev/null || true
kubectl delete job dns-test -n "$NS" 2>/dev/null || true
kubectl delete pod e2e-pvc-test -n "$NS" 2>/dev/null || true
kubectl delete pvc e2e-test-pvc -n "$NS" 2>/dev/null || true

# Verify clean — no Helm releases for our project remain
REMAINING_RELEASES=$( (helm list -q 2>/dev/null || true) | grep -c "^${PROJECT_NAME}" || true)
if [ "$REMAINING_RELEASES" -eq 0 ]; then
  pass "All Helm releases cleaned up"
else
  fail "$REMAINING_RELEASES Helm release(s) still present"
fi

# Delete the test namespace
kubectl delete namespace "$E2E_NS" --timeout=60s 2>/dev/null || true

# =============================================================================
# Summary
# =============================================================================
echo ""
echo "=== Results ==="
if [ "$FAILURES" -eq 0 ]; then
  echo -e "${GREEN}All ${PASS_COUNT} assertions passed.${NC}"
  exit 0
else
  echo -e "${RED}${FAILURES} assertion(s) failed, ${PASS_COUNT} passed.${NC}"
  exit 1
fi
