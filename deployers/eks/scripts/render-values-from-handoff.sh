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
  echo "$EXTRA_ENV_JSON" | jq -r 'to_entries[] | "  \(.key): \(.value | @json)"' >> "$OUT_DIR/indexer-values.yaml"
fi

# Inject user-defined secret env vars from handoff (stored in K8s Secret, referenced via secretKeyRef)
EXTRA_SECRET_ENV_JSON=$(jq -c '.services.indexer.extra_secret_env // {}' "$HANDOFF_FILE")
if [[ "$EXTRA_SECRET_ENV_JSON" != "{}" && "$EXTRA_SECRET_ENV_JSON" != "null" ]]; then
  echo "extraSecretEnv:" >> "$OUT_DIR/indexer-values.yaml"
  echo "$EXTRA_SECRET_ENV_JSON" | jq -r 'to_entries[] | "  \(.key): \(.value | @json)"' >> "$OUT_DIR/indexer-values.yaml"
fi

# --- Custom service values ---

CUSTOM_SERVICES=$(jq -c '.services.custom_services // null' "$HANDOFF_FILE")

if [[ "$CUSTOM_SERVICES" != "null" && "$CUSTOM_SERVICES" != "[]" ]]; then
  for SVC in $(echo "$CUSTOM_SERVICES" | jq -c '.[]'); do
    SVC_NAME=$(echo "$SVC" | jq -r '.name')
    SVC_IMAGE=$(echo "$SVC" | jq -r '.image')
    SVC_IMAGE_REPO="${SVC_IMAGE%%:*}"
    SVC_IMAGE_TAG="${SVC_IMAGE##*:}"
    if [[ "$SVC_IMAGE_REPO" == "$SVC_IMAGE_TAG" ]]; then SVC_IMAGE_TAG="latest"; fi

    cat > "$OUT_DIR/custom-${SVC_NAME}-values.yaml" <<EOF
fullnameOverride: ${PROJECT}-${SVC_NAME}
image:
  repository: ${SVC_IMAGE_REPO}
  tag: "${SVC_IMAGE_TAG}"
replicas: $(echo "$SVC" | jq -r '.replicas // 1')
service:
  port: $(echo "$SVC" | jq -r '.port')
healthPath: "$(echo "$SVC" | jq -r '.health_path // "/health"')"
resources:
  requests:
    cpu: $(echo "$SVC" | jq -r '.cpu_request // "250m"')
    memory: $(echo "$SVC" | jq -r '.memory_request // "256Mi"')
  limits:
    cpu: $(echo "$SVC" | jq -r '.cpu_limit // "500m"')
    memory: $(echo "$SVC" | jq -r '.memory_limit // "512Mi"')
enableEgress: $(echo "$SVC" | jq -r '.enable_egress // false')
EOF

    # nodeSelector
    SVC_NODE_ROLE=$(echo "$SVC" | jq -r '.node_role // empty')
    if [[ -n "$SVC_NODE_ROLE" && "$SVC_NODE_ROLE" != "null" ]]; then
      cat >> "$OUT_DIR/custom-${SVC_NAME}-values.yaml" <<EOF
nodeSelector:
  evm-cloud/role: "${SVC_NODE_ROLE}"
EOF
    fi

    # tolerations
    SVC_TOLERATIONS=$(echo "$SVC" | jq -c '.tolerations // []')
    if [[ "$SVC_TOLERATIONS" != "[]" && "$SVC_TOLERATIONS" != "null" ]]; then
      echo "tolerations:" >> "$OUT_DIR/custom-${SVC_NAME}-values.yaml"
      echo "$SVC_TOLERATIONS" | jq -r '.[] | "  - key: \(.key)\n    operator: \(.operator // "Equal")\n    value: \(.value // "")\n    effect: \(.effect // "NoSchedule")"' >> "$OUT_DIR/custom-${SVC_NAME}-values.yaml"
    fi

    # User env vars
    SVC_EXTRA_ENV=$(echo "$SVC" | jq -c '.env // {}')
    if [[ "$SVC_EXTRA_ENV" != "{}" && "$SVC_EXTRA_ENV" != "null" ]]; then
      echo "extraEnv:" >> "$OUT_DIR/custom-${SVC_NAME}-values.yaml"
      echo "$SVC_EXTRA_ENV" | jq -r 'to_entries[] | "  \(.key): \(.value | @json)"' >> "$OUT_DIR/custom-${SVC_NAME}-values.yaml"
    fi

    SVC_EXTRA_SECRET_ENV=$(echo "$SVC" | jq -c '.secret_env // {}')
    if [[ "$SVC_EXTRA_SECRET_ENV" != "{}" && "$SVC_EXTRA_SECRET_ENV" != "null" ]]; then
      echo "extraSecretEnv:" >> "$OUT_DIR/custom-${SVC_NAME}-values.yaml"
      echo "$SVC_EXTRA_SECRET_ENV" | jq -r 'to_entries[] | "  \(.key): \(.value | @json)"' >> "$OUT_DIR/custom-${SVC_NAME}-values.yaml"
    fi

    echo "Wrote $OUT_DIR/custom-${SVC_NAME}-values.yaml"
  done
fi

echo "Wrote $OUT_DIR/rpc-proxy-values.yaml"
echo "Wrote $OUT_DIR/indexer-values.yaml"
