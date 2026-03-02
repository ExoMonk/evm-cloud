#!/usr/bin/env bash
# Renders Helm values from a workload_handoff JSON file.
# Writes rpc-proxy-values.yaml and indexer-values.yaml to OUT_DIR.
# These are then populated with real configs by populate-values-from-config-bundle.sh.
#
# Usage: render-values.sh <handoff.json> <out-dir>
set -euo pipefail

HANDOFF="${1:?Usage: render-values.sh <handoff.json> <out-dir>}"
OUT_DIR="${2:?Usage: render-values.sh <handoff.json> <out-dir>}"

PROJECT=$(jq -r '.project_name' "$HANDOFF")
RPC_PORT=$(jq -r '.services.rpc_proxy.port // 4000' "$HANDOFF")
BACKEND=$(jq -r '.data.backend // "postgres"' "$HANDOFF")

# Extract RPC URL: if rpc_proxy is enabled, use its internal service URL
RPC_INTERNAL_URL=$(jq -r '.services.rpc_proxy.internal_url // empty' "$HANDOFF")
if [[ -n "$RPC_INTERNAL_URL" ]]; then
  RPC_URL="$RPC_INTERNAL_URL"
else
  # k3s: eRPC runs as a ClusterIP service in the same cluster
  RPC_PROXY_ENABLED=$(jq -r '.services.rpc_proxy != null' "$HANDOFF")
  if [[ "$RPC_PROXY_ENABLED" == "true" ]]; then
    RPC_URL="http://${PROJECT}-erpc:${RPC_PORT}"
  else
    RPC_URL=""
  fi
fi

# Extract database credentials from handoff
CH_URL=$(jq -r '.data.clickhouse.url // empty' "$HANDOFF")
CH_USER=$(jq -r '.data.clickhouse.user // "default"' "$HANDOFF")
CH_DB=$(jq -r '.data.clickhouse.db // "default"' "$HANDOFF")
CH_PASSWORD=$(jq -r '.data.clickhouse.password // empty' "$HANDOFF")
PG_URL=$(jq -r '.data.postgres.url // empty' "$HANDOFF")

mkdir -p "$OUT_DIR"

cat > "$OUT_DIR/rpc-proxy-values.yaml" <<EOF
fullnameOverride: ${PROJECT}-erpc
service:
  port: ${RPC_PORT}
config:
  erpcYaml: |
    # paste erpc.yaml content here
    logLevel: info
    server:
      listenV4: true
      httpHostV4: 0.0.0.0
      httpPort: ${RPC_PORT}
    projects: []
EOF

cat > "$OUT_DIR/indexer-values.yaml" <<EOF
fullnameOverride: ${PROJECT}-indexer
storageBackend: ${BACKEND}
replicas: 1
strategy:
  type: Recreate
rpcUrl: "${RPC_URL}"
config:
  rindexerYaml: |
    # paste rindexer.yaml content here
    name: ${PROJECT}
    project_type: no-code
  abis: {}
postgres:
  databaseUrl: "${PG_URL}"
clickhouse:
  url: "${CH_URL}"
  user: "${CH_USER}"
  db: "${CH_DB}"
  password: "${CH_PASSWORD}"
EOF

echo "Wrote $OUT_DIR/rpc-proxy-values.yaml"
echo "Wrote $OUT_DIR/indexer-values.yaml"
