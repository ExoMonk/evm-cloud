#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

info() { echo -e "${BLUE}[info]${NC}  $*"; }
ok()   { echo -e "${GREEN}[ok]${NC}    $*"; }

info "Tearing down local stack..."
"$SCRIPT_DIR/down.sh"

# Clean persistent data if it exists
if [[ -d "${HOME}/.evm-cloud/local-data" ]]; then
  info "Clearing persistent data at ~/.evm-cloud/local-data/"
  rm -rf "${HOME}/.evm-cloud/local-data"
  ok "Persistent data cleared."
fi

# Clean generated values
rm -f "$SCRIPT_DIR/values/erpc-values.yaml" "$SCRIPT_DIR/values/indexer-values.yaml"

info "Restarting local stack..."
"$SCRIPT_DIR/up.sh" "$@"
