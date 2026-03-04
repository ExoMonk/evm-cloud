#!/usr/bin/env bash
set -euo pipefail

CLUSTER_NAME="evm-cloud-local"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

err()  { echo -e "${RED}[error]${NC} $*" >&2; }

# Check cluster exists
if ! kind get clusters 2>/dev/null | grep -q "$CLUSTER_NAME"; then
  err "No $CLUSTER_NAME cluster found. Run ./local/up.sh first."
  exit 1
fi

kubectl config use-context "kind-${CLUSTER_NAME}" &>/dev/null

# Check service health
check_health() {
  local name=$1
  local url=$2
  if curl -sf --max-time 2 "$url" &>/dev/null; then
    echo -e "  ${name}$(printf '%*s' $((14 - ${#name})) '')${GREEN}Running${NC}   ${url}"
  else
    echo -e "  ${name}$(printf '%*s' $((14 - ${#name})) '')${RED}Down${NC}      ${url}"
  fi
}

echo ""
echo -e "${BLUE}evm-cloud local stack${NC} — $CLUSTER_NAME cluster"
echo ""
echo "  Service       Status    Endpoint"
echo "  ─────────     ──────    ────────"

# Anvil needs JSON-RPC POST
if curl -sf --max-time 2 http://localhost:8545 -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' &>/dev/null; then
  echo -e "  Anvil         ${GREEN}Running${NC}   http://localhost:8545"
else
  echo -e "  Anvil         ${RED}Down${NC}      http://localhost:8545"
fi
check_health "eRPC" "http://localhost:4000"
check_health "ClickHouse" "http://localhost:8123/ping"
check_health "rindexer" "http://localhost:18080/health"

# Check if Grafana is deployed (only show if monitoring services exist)
if kubectl get svc -l app.kubernetes.io/name=grafana --no-headers 2>/dev/null | grep -q grafana; then
  check_health "Grafana" "http://localhost:3000"
else
  echo -e "  Grafana       ${YELLOW}—${NC}         (not enabled)"
fi

echo ""

# Chain info
FORK_URL=$(kubectl get deploy -l app.kubernetes.io/name=anvil -o jsonpath='{.items[0].spec.template.spec.containers[0].args}' 2>/dev/null | grep -o 'fork-url=[^ "]*' | cut -d= -f2 || true)
echo "  Chain ID: 31337 (Anvil)"
if [[ -n "$FORK_URL" ]]; then
  echo "  Mode: fork ($FORK_URL)"
else
  echo "  Mode: fresh (no fork)"
fi
echo ""

# Anvil accounts
ANVIL_RESPONSE=$(curl -sf http://localhost:8545 -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_accounts","params":[],"id":1}' 2>/dev/null || echo "")
if [[ -n "$ANVIL_RESPONSE" ]]; then
  echo "Accounts (Anvil defaults):"
  echo "$ANVIL_RESPONSE" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    for i, addr in enumerate(data.get('result', [])[:5]):
        print(f'  ({i}) {addr} (10000 ETH)')
    total = len(data.get('result', []))
    if total > 5:
        print(f'  ... ({total - 5} more)')
except: pass
" 2>/dev/null || true
  echo ""
fi

# Indexed tables
echo "Indexed tables:"
TABLES=$(curl -sf 'http://localhost:8123/?user=default&password=local-dev' --data-binary "SELECT database || '.' || name FROM system.tables WHERE database NOT IN ('system', 'information_schema', 'INFORMATION_SCHEMA') ORDER BY database, name" 2>/dev/null || echo "")
if [[ -z "$TABLES" ]]; then
  echo "  (none — add contracts to local/config/rindexer.yaml)"
else
  echo "$TABLES" | while IFS= read -r table; do
    [[ -n "$table" ]] && echo "  - $table"
  done
fi
echo ""

echo "Quick start:"
echo "  # Deploy a contract"
echo "  forge create src/MyContract.sol:MyContract --rpc-url http://localhost:8545"
echo ""
echo "  # Tear down"
echo "  ./local/down.sh"
