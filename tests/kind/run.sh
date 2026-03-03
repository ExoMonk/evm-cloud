#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Local K8s validation for EKS modules using kind
# Creates a throwaway cluster, applies EKS K8s modules, asserts resource shape,
# validates Helm charts, and tears down.
# =============================================================================

CLUSTER_NAME="evm-cloud-test"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
KUBECONFIG_PATH="${SCRIPT_DIR}/.kubeconfig"
CHARTS_DIR="${REPO_ROOT}/deployers/charts"
SCRIPTS_DIR="${REPO_ROOT}/deployers/eks/scripts"
HELM_NS="helm-test"

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
  helm uninstall test-rpc -n "$HELM_NS" 2>/dev/null || true
  helm uninstall test-idx -n "$HELM_NS" 2>/dev/null || true
  kubectl delete namespace "$HELM_NS" 2>/dev/null || true
  terraform destroy -auto-approve \
    -var="kubeconfig_path=${KUBECONFIG_PATH}" 2>/dev/null || true
  kind delete cluster --name "$CLUSTER_NAME" 2>/dev/null || true
  rm -f "$KUBECONFIG_PATH"
  rm -rf "${SCRIPT_DIR}/.test-deployer"
}
trap cleanup EXIT INT TERM

# --- Phase 0: Prerequisites ---
echo "=== Phase 0: Prerequisites ==="
MISSING=""
for cmd in kind kubectl helm terraform jq python3; do
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

# 13. Helm provider initialized
if terraform providers 2>/dev/null | grep -q "hashicorp/helm"; then
  pass "Helm provider initialized in Terraform state"
else
  fail "Helm provider not found in Terraform state"
fi

# 14. Helm smoke test release deployed
SMOKE_STATUS=$(kubectl get deploy helm-smoke-test-hello-world -n default -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
if [ "${SMOKE_STATUS:-0}" -ge 1 ] 2>/dev/null; then
  pass "Helm smoke test deployment running"
else
  fail "Helm smoke test deployment not ready (replicas: ${SMOKE_STATUS:-0})"
fi

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

# 5c. Ingress: Cloudflare mode dry-run
if helm template test-rpc-ingress-cf "$CHARTS_DIR/rpc-proxy" \
    --values "$CHARTS_DIR/rpc-proxy/values.yaml" \
    --set ingress.enabled=true \
    --set ingress.host=test.example.com \
    --set ingress.tlsProvider=cloudflare \
    --set ingress.tlsSecretName=cloudflare-origin-tls 2>&1 \
    | kubectl apply --dry-run=server -f - >/dev/null 2>&1; then
  pass "rpc-proxy Ingress (Cloudflare mode) renders and validates"
else
  fail "rpc-proxy Ingress (Cloudflare mode) dry-run failed"
fi

# 5d. Ingress: cert-manager mode dry-run
if helm template test-rpc-ingress-cm "$CHARTS_DIR/rpc-proxy" \
    --values "$CHARTS_DIR/rpc-proxy/values.yaml" \
    --set ingress.enabled=true \
    --set ingress.host=test.example.com \
    --set ingress.tlsProvider=cert-manager \
    --set ingress.clusterIssuer=letsencrypt-staging 2>&1 \
    | kubectl apply --dry-run=server -f - >/dev/null 2>&1; then
  pass "rpc-proxy Ingress (cert-manager mode) renders and validates"
else
  fail "rpc-proxy Ingress (cert-manager mode) dry-run failed"
fi

# Verify cert-manager annotation is present in the cert-manager template
CM_RENDERED=$(helm template test-rpc-cm "$CHARTS_DIR/rpc-proxy" \
    --values "$CHARTS_DIR/rpc-proxy/values.yaml" \
    --set ingress.enabled=true \
    --set ingress.host=test.example.com \
    --set ingress.tlsProvider=cert-manager \
    --set ingress.clusterIssuer=letsencrypt-staging 2>&1)

if echo "$CM_RENDERED" | grep -q "cert-manager.io/cluster-issuer"; then
  pass "cert-manager Ingress has cluster-issuer annotation"
else
  fail "cert-manager Ingress missing cluster-issuer annotation"
fi

if echo "$CM_RENDERED" | grep -q "ingressClassName: nginx"; then
  pass "Ingress has ingressClassName: nginx"
else
  fail "Ingress missing ingressClassName"
fi

# --- Phase 6: Deployer scripts pipeline test ---
echo ""
echo "=== Phase 6: Deployer scripts pipeline ==="

TEST_DIR="${SCRIPT_DIR}/.test-deployer"
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR/values" "$TEST_DIR/config/abis"

# Create test handoff JSON
cat > "$TEST_DIR/handoff.json" <<'HANDOFF'
{
  "mode": "external",
  "compute_engine": "eks",
  "project_name": "kind-test",
  "data": { "backend": "clickhouse" },
  "services": { "rpc_proxy": { "port": 4000 } }
}
HANDOFF

# Create test config files (same content as Terraform test vars)
cat > "$TEST_DIR/config/erpc.yaml" <<'ERPC'
logLevel: info
server:
  listenV4: true
  httpHostV4: 0.0.0.0
  httpPort: 4000
projects:
  - id: main
    networks:
      - architecture: evm
        evm:
          chainId: 1
    upstreams:
      - id: public
        endpoint: https://ethereum-rpc.publicnode.com
        type: evm
ERPC

cat > "$TEST_DIR/config/rindexer.yaml" <<'RINDEXER'
name: kind-test-indexer
project_type: no-code
networks:
  - name: ethereum
    chain_id: 1
    rpc: http://localhost:8545
storage:
  clickhouse:
    enabled: true
contracts: []
RINDEXER

echo '{"abi": []}' > "$TEST_DIR/config/abis/ERC20.json"

# 6a. render-values-from-handoff.sh
if bash "$SCRIPTS_DIR/render-values-from-handoff.sh" "$TEST_DIR/handoff.json" "$TEST_DIR/values" >/dev/null 2>&1; then
  pass "render-values-from-handoff.sh exits 0"
else
  fail "render-values-from-handoff.sh failed"
fi

if [ -f "$TEST_DIR/values/rpc-proxy-values.yaml" ] && [ -f "$TEST_DIR/values/indexer-values.yaml" ]; then
  pass "render script created both values files"
else
  fail "render script did not create expected values files"
fi

if grep -q "kind-test-erpc" "$TEST_DIR/values/rpc-proxy-values.yaml"; then
  pass "rpc-proxy values contain project-derived fullnameOverride"
else
  fail "rpc-proxy values missing fullnameOverride"
fi

# 6b. populate-values-from-config-bundle.sh
if bash "$SCRIPTS_DIR/populate-values-from-config-bundle.sh" \
    --values-dir "$TEST_DIR/values" --config-dir "$TEST_DIR/config" >/dev/null 2>&1; then
  pass "populate-values-from-config-bundle.sh exits 0"
else
  fail "populate-values-from-config-bundle.sh failed"
fi

if grep -q "httpPort: 4000" "$TEST_DIR/values/rpc-proxy-values.yaml" && \
   grep -q "publicnode.com" "$TEST_DIR/values/rpc-proxy-values.yaml"; then
  pass "populated rpc-proxy values contain actual erpc.yaml content"
else
  fail "populated rpc-proxy values missing erpc config content"
fi

if grep -q "kind-test-indexer" "$TEST_DIR/values/indexer-values.yaml" && \
   grep -q "ERC20.json" "$TEST_DIR/values/indexer-values.yaml"; then
  pass "populated indexer values contain rindexer config + ABIs"
else
  fail "populated indexer values missing config or ABIs"
fi

# 6c. Error case: invalid handoff (wrong mode)
cat > "$TEST_DIR/bad-handoff.json" <<'BAD'
{
  "mode": "terraform",
  "compute_engine": "eks",
  "project_name": "bad-test"
}
BAD

if bash "$SCRIPTS_DIR/render-values-from-handoff.sh" "$TEST_DIR/bad-handoff.json" "$TEST_DIR/bad-values" >/dev/null 2>&1; then
  fail "render script should reject mode=terraform handoff"
else
  pass "render script correctly rejects invalid handoff (mode=terraform)"
fi

# 6d. k3s render-values.sh: Cloudflare ingress handoff
K3S_RENDER="${REPO_ROOT}/deployers/k3s/scripts/render-values.sh"
mkdir -p "$TEST_DIR/ingress-cf-values"
cat > "$TEST_DIR/ingress-cf-handoff.json" <<'INGCF'
{
  "mode": "external",
  "compute_engine": "k3s",
  "project_name": "ingress-test",
  "services": { "rpc_proxy": { "port": 4000 } },
  "data": { "backend": "clickhouse", "clickhouse": { "url": "http://localhost:8123", "user": "default", "password": "test", "db": "default" } },
  "ingress": {
    "mode": "cloudflare",
    "domain": "rpc.test.example.com",
    "nginx_chart_version": "4.11.3",
    "cloudflare": { "origin_cert": "FAKE_CERT_PEM", "origin_key": "FAKE_KEY_PEM" }
  }
}
INGCF

if bash "$K3S_RENDER" "$TEST_DIR/ingress-cf-handoff.json" "$TEST_DIR/ingress-cf-values" >/dev/null 2>&1; then
  pass "k3s render-values.sh handles Cloudflare ingress handoff"
else
  fail "k3s render-values.sh failed on Cloudflare ingress handoff"
fi

CF_VALUES="$TEST_DIR/ingress-cf-values/rpc-proxy-values.yaml"
if grep -q "enabled: true" "$CF_VALUES" && grep -q "rpc.test.example.com" "$CF_VALUES"; then
  pass "Cloudflare ingress values: enabled + correct host"
else
  fail "Cloudflare ingress values missing enabled/host"
fi

if grep -q 'tlsProvider: "cloudflare"' "$CF_VALUES"; then
  pass "Cloudflare ingress values: tlsProvider=cloudflare"
else
  fail "Cloudflare ingress values missing tlsProvider"
fi

if grep -q 'tlsSecretName: "cloudflare-origin-tls"' "$CF_VALUES"; then
  pass "Cloudflare ingress values: correct tlsSecretName"
else
  fail "Cloudflare ingress values missing tlsSecretName"
fi

# 6e. k3s render-values.sh: ingress_nginx mode
mkdir -p "$TEST_DIR/ingress-nginx-values"
cat > "$TEST_DIR/ingress-nginx-handoff.json" <<'INGNX'
{
  "mode": "external",
  "compute_engine": "k3s",
  "project_name": "ingress-test",
  "services": { "rpc_proxy": { "port": 4000 } },
  "data": { "backend": "clickhouse", "clickhouse": { "url": "http://localhost:8123", "user": "default", "password": "test", "db": "default" } },
  "ingress": {
    "mode": "ingress_nginx",
    "domain": "rpc.test.example.com",
    "tls_email": "test@example.com",
    "tls_staging": true,
    "cert_manager_chart_version": "1.16.2"
  }
}
INGNX

if bash "$K3S_RENDER" "$TEST_DIR/ingress-nginx-handoff.json" "$TEST_DIR/ingress-nginx-values" >/dev/null 2>&1; then
  pass "k3s render-values.sh handles ingress_nginx handoff"
else
  fail "k3s render-values.sh failed on ingress_nginx handoff"
fi

NX_VALUES="$TEST_DIR/ingress-nginx-values/rpc-proxy-values.yaml"
if grep -q 'tlsProvider: "cert-manager"' "$NX_VALUES"; then
  pass "ingress_nginx values: tlsProvider=cert-manager"
else
  fail "ingress_nginx values missing tlsProvider"
fi

if grep -q 'clusterIssuer: "letsencrypt-staging"' "$NX_VALUES"; then
  pass "ingress_nginx values: clusterIssuer=letsencrypt-staging (staging mode)"
else
  fail "ingress_nginx values missing staging clusterIssuer"
fi

# --- Phase 7: Helm install + assertions ---
echo ""
echo "=== Phase 7: Helm install + assertions ==="

kubectl create namespace "$HELM_NS" 2>/dev/null || true

# Write test values matching Phase 2 config (same eRPC/rindexer/ABIs/creds)
cat > "$TEST_DIR/helm-rpc-proxy-values.yaml" <<'HELMRPC'
fullnameOverride: helm-test-erpc
service:
  port: 4000
config:
  erpcYaml: |
    logLevel: info
    server:
      listenV4: true
      httpHostV4: 0.0.0.0
      httpPort: 4000
    projects:
      - id: main
        networks:
          - architecture: evm
            evm:
              chainId: 1
        upstreams:
          - id: public
            endpoint: https://ethereum-rpc.publicnode.com
            type: evm
HELMRPC

cat > "$TEST_DIR/helm-indexer-values.yaml" <<'HELMIDX'
fullnameOverride: helm-test-indexer
storageBackend: clickhouse
rpcUrl: "http://localhost:8545"
clickhouse:
  url: "http://localhost:8123"
  user: "default"
  db: "test_db"
  password: "test-password"
config:
  rindexerYaml: |
    name: helm-test-indexer
    project_type: no-code
    networks:
      - name: ethereum
        chain_id: 1
        rpc: http://localhost:8545
    storage:
      clickhouse:
        enabled: true
    contracts: []
  abis:
    ERC20.json: |-
      {"abi": []}
HELMIDX

helm install test-rpc "$CHARTS_DIR/rpc-proxy" \
  -n "$HELM_NS" -f "$TEST_DIR/helm-rpc-proxy-values.yaml" --wait=false

helm install test-idx "$CHARTS_DIR/indexer" \
  -n "$HELM_NS" -f "$TEST_DIR/helm-indexer-values.yaml" --wait=false

# Give K8s a moment to create resources
sleep 3

# 7.1 rpc-proxy ConfigMap
if kubectl get configmap helm-test-erpc-config -n "$HELM_NS" -o jsonpath='{.data.erpc\.yaml}' 2>/dev/null | grep -q "httpPort: 4000"; then
  pass "[helm] rpc-proxy ConfigMap contains erpc.yaml with httpPort 4000"
else
  fail "[helm] rpc-proxy ConfigMap missing or malformed"
fi

# 7.2 rpc-proxy Deployment image
if kubectl get deployment helm-test-erpc -n "$HELM_NS" -o jsonpath='{.spec.template.spec.containers[0].image}' 2>/dev/null | grep -q "erpc"; then
  pass "[helm] rpc-proxy Deployment uses erpc image"
else
  fail "[helm] rpc-proxy Deployment missing or wrong image"
fi

# 7.3 rpc-proxy Service port
if kubectl get service helm-test-erpc -n "$HELM_NS" -o jsonpath='{.spec.ports[0].port}' 2>/dev/null | grep -q "4000"; then
  pass "[helm] rpc-proxy Service exposes port 4000"
else
  fail "[helm] rpc-proxy Service missing or wrong port"
fi

# 7.4 rpc-proxy Service type
SVC_TYPE=$(kubectl get service helm-test-erpc -n "$HELM_NS" -o jsonpath='{.spec.type}' 2>/dev/null)
if [ "$SVC_TYPE" = "ClusterIP" ]; then
  pass "[helm] rpc-proxy Service type is ClusterIP"
else
  fail "[helm] rpc-proxy Service type: expected ClusterIP, got $SVC_TYPE"
fi

# 7.5 indexer ConfigMap (config)
if kubectl get configmap helm-test-indexer-config -n "$HELM_NS" -o jsonpath='{.data.rindexer\.yaml}' 2>/dev/null | grep -q "helm-test-indexer"; then
  pass "[helm] indexer ConfigMap contains rindexer.yaml"
else
  fail "[helm] indexer ConfigMap missing or malformed"
fi

# 7.6 indexer ConfigMap (ABIs)
if kubectl get configmap helm-test-indexer-abis -n "$HELM_NS" -o jsonpath='{.data.ERC20\.json}' 2>/dev/null | grep -q "abi"; then
  pass "[helm] indexer ABIs ConfigMap contains ERC20.json"
else
  fail "[helm] indexer ABIs ConfigMap missing or malformed"
fi

# 7.7 indexer Secret
if kubectl get secret helm-test-indexer-secrets -n "$HELM_NS" -o jsonpath='{.data.CLICKHOUSE_PASSWORD}' 2>/dev/null | base64 -d 2>/dev/null | grep -q "test-password"; then
  pass "[helm] indexer Secret contains CLICKHOUSE_PASSWORD"
else
  fail "[helm] indexer Secret missing or wrong value"
fi

# 7.8 indexer Deployment strategy
STRATEGY=$(kubectl get deployment helm-test-indexer -n "$HELM_NS" -o jsonpath='{.spec.strategy.type}' 2>/dev/null)
if [ "$STRATEGY" = "Recreate" ]; then
  pass "[helm] indexer Deployment uses Recreate strategy (single-writer)"
else
  fail "[helm] indexer Deployment strategy: expected Recreate, got $STRATEGY"
fi

# 7.9 indexer Deployment replicas
REPLICAS=$(kubectl get deployment helm-test-indexer -n "$HELM_NS" -o jsonpath='{.spec.replicas}' 2>/dev/null)
if [ "$REPLICAS" = "1" ]; then
  pass "[helm] indexer Deployment has 1 replica"
else
  fail "[helm] indexer Deployment replicas: expected 1, got $REPLICAS"
fi

# 7.10 indexer env vars
CONTAINER_ENV=$(kubectl get deployment helm-test-indexer -n "$HELM_NS" -o jsonpath='{.spec.template.spec.containers[0].env[*].name}' 2>/dev/null)
for expected_var in RPC_URL CLICKHOUSE_URL CLICKHOUSE_USER CLICKHOUSE_DB CLICKHOUSE_PASSWORD; do
  if echo "$CONTAINER_ENV" | grep -q "$expected_var"; then
    pass "[helm] indexer Deployment has env var $expected_var"
  else
    fail "[helm] indexer Deployment missing env var $expected_var"
  fi
done

# 7.11 indexer volume mounts (config + abis)
VOLUME_NAMES=$(kubectl get deployment helm-test-indexer -n "$HELM_NS" -o jsonpath='{.spec.template.spec.volumes[*].name}' 2>/dev/null)
for expected_vol in config abis; do
  if echo "$VOLUME_NAMES" | grep -q "$expected_vol"; then
    pass "[helm] indexer Deployment has volume $expected_vol"
  else
    fail "[helm] indexer Deployment missing volume $expected_vol"
  fi
done

# --- Phase 8: Helm runtime validation ---
echo ""
echo "=== Phase 8: Helm runtime validation ==="

# --- Helm eRPC: should fully start ---
echo "  Waiting for Helm eRPC deployment to be available (up to 180s)..."
if kubectl wait --for=condition=available deployment/helm-test-erpc -n "$HELM_NS" --timeout=180s 2>/dev/null; then
  ERPC_POD=$(kubectl get pods -l "app.kubernetes.io/instance=test-rpc" -n "$HELM_NS" -o jsonpath='{.items[0].metadata.name}' 2>/dev/null)
  POD_PHASE=$(kubectl get pod "$ERPC_POD" -n "$HELM_NS" -o jsonpath='{.status.phase}' 2>/dev/null)
  pass "[helm] eRPC deployment available (pod: $POD_PHASE)"

  if [ "$POD_PHASE" = "Running" ]; then
    pass "[helm] eRPC pod is Running"

    ERPC_LOGS=$(kubectl logs "$ERPC_POD" -n "$HELM_NS" --tail=20 2>/dev/null || echo "")
    if [ -n "$ERPC_LOGS" ]; then
      pass "[helm] eRPC produced logs"
      echo "    (last log: $(echo "$ERPC_LOGS" | tail -1 | cut -c1-120))"
    else
      fail "[helm] eRPC is Running but produced no logs"
    fi

    # Port-forward on different port to avoid collision with Phase 4
    kubectl port-forward "service/helm-test-erpc" 14001:4000 -n "$HELM_NS" >/dev/null 2>&1 &
    ERPC_PF_PID=$!
    sleep 3

    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:14001/ 2>/dev/null || echo "000")
    if [ "$HTTP_CODE" != "000" ]; then
      pass "[helm] eRPC HTTP responds (status $HTTP_CODE)"
    else
      fail "[helm] eRPC did not respond to HTTP request"
    fi

    kill "$ERPC_PF_PID" 2>/dev/null || true
    wait "$ERPC_PF_PID" 2>/dev/null || true
  else
    fail "[helm] eRPC pod did not reach Running (stuck in $POD_PHASE)"
  fi
else
  fail "[helm] eRPC deployment did not become available within 180s"
fi

# --- Helm rindexer: will crash on ClickHouse connect, but should pull image + attempt start ---
echo "  Waiting for Helm rindexer pod (image pull + start, up to 120s)..."
HELM_INDEXER_POD="" HELM_INDEXER_PHASE=""
ELAPSED=0
while [ $ELAPSED -lt 120 ]; do
  HELM_INDEXER_POD=$(kubectl get pods -l "app.kubernetes.io/instance=test-idx" -n "$HELM_NS" -o jsonpath='{.items[0].metadata.name}' 2>/dev/null)
  if [ -n "$HELM_INDEXER_POD" ]; then
    STATE=$(kubectl get pod "$HELM_INDEXER_POD" -n "$HELM_NS" -o jsonpath='{.status.containerStatuses[0].state}' 2>/dev/null || echo "")
    if [ -n "$STATE" ]; then
      HELM_INDEXER_PHASE=$(kubectl get pod "$HELM_INDEXER_POD" -n "$HELM_NS" -o jsonpath='{.status.phase}' 2>/dev/null)
      break
    fi
  fi
  sleep 5
  ELAPSED=$((ELAPSED + 5))
done

if [ -n "$HELM_INDEXER_POD" ] && [ -n "$HELM_INDEXER_PHASE" ]; then
  pass "[helm] rindexer pod created (phase: $HELM_INDEXER_PHASE)"

  sleep 3
  INDEXER_LOGS=$(kubectl logs "$HELM_INDEXER_POD" -n "$HELM_NS" --tail=30 2>/dev/null || echo "")
  if [ -n "$INDEXER_LOGS" ]; then
    pass "[helm] rindexer produced logs (container ran)"
    echo "    (last log: $(echo "$INDEXER_LOGS" | tail -1 | cut -c1-120))"
  else
    fail "[helm] rindexer pod started but produced no logs"
  fi
else
  fail "[helm] rindexer pod did not start within 120s"
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
