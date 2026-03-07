#!/usr/bin/env bash
set -euo pipefail

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required" >&2
  exit 1
fi

HANDOFF_FILE="${1:-}"
OUT_DIR="${2:-deployers/eks/values/dev}"

if [[ -z "${HANDOFF_FILE}" ]]; then
  echo "Usage: $0 <workload_handoff.json> [out_dir]" >&2
  exit 1
fi

MODE=$(jq -r '.mode' "$HANDOFF_FILE")
ENGINE=$(jq -r '.compute_engine' "$HANDOFF_FILE")
PROJECT=$(jq -r '.project_name' "$HANDOFF_FILE")
BACKEND=$(jq -r '.data.backend // "postgres"' "$HANDOFF_FILE")
RPC_PORT=$(jq -r '.services.rpc_proxy.port // 4000' "$HANDOFF_FILE")

if [[ "$MODE" != "external" || "$ENGINE" != "eks" ]]; then
  echo "handoff must be mode=external and compute_engine=eks" >&2
  exit 1
fi

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
rpcUrl: ""
config:
  rindexerYaml: |
    # paste rindexer.yaml content here
    name: ${PROJECT}
    project_type: no-code
  abis: {}
postgres:
  databaseUrl: ""
clickhouse:
  url: ""
  user: "default"
  db: "default"
  password: ""
EOF

# Inject user-defined extra env vars from handoff
EXTRA_ENV_JSON=$(jq -c '.services.indexer.extra_env // {}' "$HANDOFF_FILE")
if [[ "$EXTRA_ENV_JSON" != "{}" && "$EXTRA_ENV_JSON" != "null" ]]; then
  echo "extraEnv:" >> "$OUT_DIR/indexer-values.yaml"
  echo "$EXTRA_ENV_JSON" | jq -r 'to_entries[] | "  \(.key): \"\(.value)\""' >> "$OUT_DIR/indexer-values.yaml"
fi

echo "Wrote $OUT_DIR/rpc-proxy-values.yaml"
echo "Wrote $OUT_DIR/indexer-values.yaml"
