#!/usr/bin/env bash
set -euo pipefail

CLUSTER_NAME="evm-cloud-local"

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() { echo -e "${BLUE}[info]${NC}  $*"; }
ok()   { echo -e "${GREEN}[ok]${NC}    $*"; }
warn() { echo -e "${YELLOW}[warn]${NC}  $*"; }

if ! kind get clusters 2>/dev/null | grep -q "$CLUSTER_NAME"; then
  warn "No $CLUSTER_NAME cluster found. Nothing to do."
  exit 0
fi

info "Deleting kind cluster: $CLUSTER_NAME"
kind delete cluster --name "$CLUSTER_NAME"
ok "Local stack removed."
