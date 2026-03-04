#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CLUSTER_NAME="evm-cloud-local"
PROFILE="default"
PERSIST=false
FORCE=false
WITH_MONITORING=false
ANVIL_FORK_URL=""
POST_DEPLOY=""

# ─── Argument parsing ───────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)       PROFILE="$2"; shift 2 ;;
    --persist)       PERSIST=true; shift ;;
    --force)         FORCE=true; shift ;;
    --with-monitoring) WITH_MONITORING=true; shift ;;
    --anvil-fork)    ANVIL_FORK_URL="$2"; shift 2 ;;
    --post-deploy)   POST_DEPLOY="$2"; shift 2 ;;
    -h|--help)
      echo "Usage: ./local/up.sh [OPTIONS]"
      echo ""
      echo "Options:"
      echo "  --profile <name>     Resource profile: default, heavy (default: default)"
      echo "  --persist            Enable persistent ClickHouse data across restarts"
      echo "  --force              Force-recreate cluster even if it exists"
      echo "  --with-monitoring    Deploy Prometheus + Grafana (requires more memory)"
      echo "  --anvil-fork <url>   Fork mainnet/testnet state from RPC URL"
      echo "  --post-deploy <sh>   Script to run after stack is healthy"
      echo "  -h, --help           Show this help"
      exit 0
      ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

# ─── Colors ─────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()  { echo -e "${BLUE}[info]${NC}  $*"; }
ok()    { echo -e "${GREEN}[ok]${NC}    $*"; }
warn()  { echo -e "${YELLOW}[warn]${NC}  $*"; }
err()   { echo -e "${RED}[error]${NC} $*" >&2; }

# ─── Phase 0: Prerequisites ────────────────────────────────────────
info "Checking prerequisites..."

for cmd in kind kubectl helm docker; do
  if ! command -v "$cmd" &>/dev/null; then
    err "$cmd is required but not installed."
    exit 1
  fi
done
ok "All prerequisites found."

# Check Docker is running
if ! docker info &>/dev/null; then
  err "Docker daemon is not running. Start Docker Desktop and retry."
  exit 1
fi
ok "Docker daemon is running."

# Check Docker memory allocation
DOCKER_MEM_BYTES=$(docker info --format '{{.MemTotal}}' 2>/dev/null || echo "0")
DOCKER_MEM_GB=$(( DOCKER_MEM_BYTES / 1073741824 ))
MIN_MEM=4
if [[ "$PROFILE" == "heavy" ]]; then
  MIN_MEM=8
fi
if [[ "$DOCKER_MEM_GB" -lt "$MIN_MEM" ]]; then
  warn "Docker has ${DOCKER_MEM_GB}GB memory allocated. Recommended: ${MIN_MEM}GB+ for '${PROFILE}' profile."
  warn "Increase in Docker Desktop → Settings → Resources → Memory."
fi

# Check port conflicts
check_port() {
  local port=$1
  if lsof -iTCP:"$port" -sTCP:LISTEN -P -n &>/dev/null; then
    err "Port $port is already in use. Free it before running up.sh."
    return 1
  fi
}

# Only check ports if cluster doesn't exist (ports will be bound by existing cluster)
if ! kind get clusters 2>/dev/null | grep -q "$CLUSTER_NAME"; then
  info "Checking port availability..."
  PORTS_OK=true
  for port in 8545 4000 8123 18080; do
    if ! check_port "$port"; then
      PORTS_OK=false
    fi
  done
  if [[ "$WITH_MONITORING" == "true" ]]; then
    check_port 3000 || PORTS_OK=false
  fi
  if [[ "$PORTS_OK" == "false" ]]; then
    exit 1
  fi
  ok "All required ports are available."
fi

# ─── Phase 1: Create kind cluster ──────────────────────────────────
if kind get clusters 2>/dev/null | grep -q "$CLUSTER_NAME"; then
  if [[ "$FORCE" == "true" ]]; then
    info "Force flag set — deleting existing cluster..."
    kind delete cluster --name "$CLUSTER_NAME"
  else
    info "Reusing existing $CLUSTER_NAME cluster."
    kubectl cluster-info --context "kind-${CLUSTER_NAME}" &>/dev/null || {
      err "Cluster exists but is unreachable. Run with --force to recreate."
      exit 1
    }
  fi
fi

if ! kind get clusters 2>/dev/null | grep -q "$CLUSTER_NAME"; then
  info "Creating kind cluster: $CLUSTER_NAME"
  KIND_CONFIG="$SCRIPT_DIR/kind-config.yaml"
  if [[ "$PERSIST" == "true" ]]; then
    KIND_CONFIG="$SCRIPT_DIR/kind-config-persist.yaml"
    mkdir -p "${HOME}/.evm-cloud/local-data"
    info "Persistence enabled — data stored in ~/.evm-cloud/local-data/"
  fi
  kind create cluster \
    --name "$CLUSTER_NAME" \
    --config "$KIND_CONFIG" \
    --wait 60s
  ok "Kind cluster created."
fi

kubectl config use-context "kind-${CLUSTER_NAME}" &>/dev/null

# ─── Phase 2: Deploy ClickHouse ────────────────────────────────────
info "Deploying ClickHouse..."
kubectl apply -f "$SCRIPT_DIR/manifests/clickhouse.yaml"
info "Waiting for ClickHouse readiness..."
kubectl wait --for=condition=Ready pod -l app=clickhouse --timeout=120s
ok "ClickHouse is ready."

# Wait for ClickHouse HTTP interface via NodePort
wait_for_http() {
  local url=$1
  local timeout=${2:-30}
  local elapsed=0
  while [[ $elapsed -lt $timeout ]]; do
    if curl -sf "$url" &>/dev/null; then
      return 0
    fi
    sleep 2
    elapsed=$((elapsed + 2))
  done
  err "Timed out waiting for $url"
  return 1
}

wait_for_http "http://localhost:8123/ping" 60
ok "ClickHouse HTTP interface accessible at localhost:8123."

# ─── Phase 3: Deploy Anvil ─────────────────────────────────────────
info "Deploying Anvil..."
ANVIL_ARGS=()
if [[ -n "$ANVIL_FORK_URL" ]]; then
  ANVIL_ARGS+=(--set "anvil.forkUrl=$ANVIL_FORK_URL")
  warn "Fork mode enabled. In rindexer.yaml, set start_block close to the fork block."
  warn "Historical eth_getLogs will be proxied to the fork RPC (may rate-limit)."
fi

helm upgrade --install local-anvil "$SCRIPT_DIR/charts/anvil/" \
  -f "$SCRIPT_DIR/profiles/${PROFILE}.yaml" \
  "${ANVIL_ARGS[@]+"${ANVIL_ARGS[@]}"}" \
  --wait --timeout 120s
ok "Anvil deployed."

# ─── Phase 4: Deploy eRPC ──────────────────────────────────────────
info "Generating eRPC values..."
ERPC_CONFIG=$(cat "$SCRIPT_DIR/config/erpc.yaml")
cat > "$SCRIPT_DIR/values/erpc-values.yaml" <<EOF
fullnameOverride: local-erpc
service:
  type: NodePort
  nodePort: 30400
  port: 4000
resources:
  requests:
    cpu: 100m
    memory: 128Mi
  limits:
    cpu: 250m
    memory: 256Mi
config:
  erpcYaml: |
$(echo "$ERPC_CONFIG" | sed 's/^/    /')
EOF

info "Deploying eRPC..."
helm upgrade --install local-erpc "$REPO_ROOT/deployers/charts/rpc-proxy/" \
  -f "$SCRIPT_DIR/values/erpc-values.yaml" \
  --wait --timeout 120s
ok "eRPC deployed."

# ─── Phase 5: Deploy rindexer ──────────────────────────────────────
info "Generating indexer values..."
RINDEXER_CONFIG=$(cat "$SCRIPT_DIR/config/rindexer.yaml")

# Build ABIs map from local/config/abis/*.json
ABIS_YAML=""
if [[ -d "$SCRIPT_DIR/config/abis" ]]; then
  for abi_file in "$SCRIPT_DIR/config/abis"/*.json; do
    [[ -f "$abi_file" ]] || continue
    abi_name=$(basename "$abi_file")
    abi_content=$(cat "$abi_file")
    ABIS_YAML="${ABIS_YAML}    ${abi_name}: '${abi_content}'
"
  done
fi

cat > "$SCRIPT_DIR/values/indexer-values.yaml" <<EOF
fullnameOverride: local-indexer
storageBackend: clickhouse
secretsMode: inline
rpcUrl: http://local-erpc:4000/local/evm/31337
clickhouse:
  url: http://clickhouse:8123
  user: default
  db: default
  password: local-dev
service:
  type: NodePort
  nodePort: 31808
  port: 8080
resources:
  requests:
    cpu: 200m
    memory: 256Mi
  limits:
    cpu: 500m
    memory: 512Mi
config:
  rindexerYaml: |
$(echo "$RINDEXER_CONFIG" | sed 's/^/    /')
  abis:
${ABIS_YAML}
EOF

info "Deploying rindexer..."
helm upgrade --install local-indexer "$REPO_ROOT/deployers/charts/indexer/" \
  -f "$SCRIPT_DIR/values/indexer-values.yaml" \
  --wait --timeout 180s
ok "rindexer deployed."

# ─── Phase 6: Health checks ────────────────────────────────────────
info "Running health checks..."
# Anvil is JSON-RPC (needs POST, not GET)
ANVIL_OK=false
for i in $(seq 1 15); do
  if curl -sf http://localhost:8545 -X POST -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' &>/dev/null; then
    ANVIL_OK=true; break
  fi
  sleep 2
done
[[ "$ANVIL_OK" == "true" ]] && ok "Anvil:      http://localhost:8545" || warn "Anvil health check timed out"
wait_for_http "http://localhost:4000" 30 && ok "eRPC:       http://localhost:4000"
wait_for_http "http://localhost:8123/ping" 10 && ok "ClickHouse: http://localhost:8123"

# ─── Phase 7: Post-deploy hook ─────────────────────────────────────
if [[ -n "$POST_DEPLOY" ]]; then
  if [[ ! -x "$POST_DEPLOY" ]]; then
    warn "Post-deploy script is not executable: $POST_DEPLOY"
    warn "Run: chmod +x $POST_DEPLOY"
  else
    info "Running post-deploy script: $POST_DEPLOY"
    ANVIL_RPC_URL=http://localhost:8545 \
    ERPC_URL=http://localhost:4000 \
    CLICKHOUSE_URL=http://localhost:8123 \
    CHAIN_ID=31337 \
      "$POST_DEPLOY"
    ok "Post-deploy script completed."
  fi
fi

# ─── Phase 8: Print summary ────────────────────────────────────────
echo ""
echo -e "${GREEN}━━━ evm-cloud local stack is ready ━━━${NC}"
echo ""
echo "  Service       Endpoint"
echo "  ─────────     ────────"
echo "  Anvil         http://localhost:8545"
echo "  eRPC          http://localhost:4000"
echo "  ClickHouse    http://localhost:8123"
echo "  rindexer      http://localhost:18080"
if [[ "$WITH_MONITORING" == "true" ]]; then
  echo "  Grafana       http://localhost:3000"
fi
echo ""
echo "  Chain ID: 31337 (Anvil)"
if [[ -n "$ANVIL_FORK_URL" ]]; then
  echo "  Mode: fork ($ANVIL_FORK_URL)"
else
  echo "  Mode: fresh (no fork)"
fi
echo ""
echo "Quick start:"
echo "  # Deploy a contract"
echo "  forge create src/MyContract.sol:MyContract --rpc-url http://localhost:8545"
echo ""
echo "  # Query ClickHouse (user: default, password: local-dev)"
echo "  curl 'http://localhost:8123/?user=default&password=local-dev' -d 'SHOW TABLES'"
echo ""
echo "  # Check status"
echo "  ./local/status.sh"
echo ""
echo "  # Tear down"
echo "  ./local/down.sh"
