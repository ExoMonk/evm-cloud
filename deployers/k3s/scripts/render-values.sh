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
SECRETS_MODE=$(jq -r '.secrets.mode // "inline"' "$HANDOFF")

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

# Extract ingress config from handoff
INGRESS_MODE=$(jq -r '.ingress.mode // "none"' "$HANDOFF")
ERPC_HOSTNAME=$(jq -r '.ingress.erpc_hostname // empty' "$HANDOFF")

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

# Ingress: configure when mode is cloudflare or ingress_nginx AND erpc_hostname is set
if [[ "$INGRESS_MODE" == "cloudflare" && -n "$ERPC_HOSTNAME" ]]; then
  cat >> "$OUT_DIR/rpc-proxy-values.yaml" <<EOF
ingress:
  enabled: true
  host: "${ERPC_HOSTNAME}"
  tlsProvider: "cloudflare"
  tlsSecretName: "cloudflare-origin-tls"
EOF
elif [[ "$INGRESS_MODE" == "ingress_nginx" && -n "$ERPC_HOSTNAME" ]]; then
  TLS_STAGING=$(jq -r '.ingress.tls_staging // false' "$HANDOFF")
  ISSUER="letsencrypt-prod"
  if [[ "$TLS_STAGING" == "true" ]]; then
    ISSUER="letsencrypt-staging"
  fi
  cat >> "$OUT_DIR/rpc-proxy-values.yaml" <<EOF
ingress:
  enabled: true
  host: "${ERPC_HOSTNAME}"
  tlsProvider: "cert-manager"
  clusterIssuer: "${ISSUER}"
EOF
fi

# Multi-node: pin eRPC to server node so it doesn't land on a worker
WORKER_COUNT=$(jq '[.runtime.k3s.worker_nodes // [] | length] | add' "$HANDOFF")
if [[ "$WORKER_COUNT" -gt 0 ]]; then
  cat >> "$OUT_DIR/rpc-proxy-values.yaml" <<EOF
nodeSelector:
  evm-cloud/role: "server"
EOF
fi

# --- Indexer values ---
# Multi-instance support: if services.indexer.instances[] exists, generate per-instance values.
# Otherwise, generate a single indexer-values.yaml (backward compat).

render_indexer_values() {
  local INSTANCE_NAME="$1"
  local NODE_ROLE="$2"
  local OUT_FILE="$3"
  local WORKLOAD_TYPE="${4:-deployment}"

  cat > "$OUT_FILE" <<EOF
fullnameOverride: ${PROJECT}-${INSTANCE_NAME}
workloadType: ${WORKLOAD_TYPE}
storageBackend: ${BACKEND}
replicas: 1
strategy:
  type: Recreate
rpcUrl: "${RPC_URL}"
secretsMode: "${SECRETS_MODE}"
config:
  rindexerYaml: |
    # paste rindexer.yaml content here
    name: ${PROJECT}
    project_type: no-code
  abis: {}
EOF

  # Secrets: inline mode includes credentials in values, provider/external uses ESO
  if [[ "$SECRETS_MODE" == "inline" ]]; then
    cat >> "$OUT_FILE" <<EOF
postgres:
  databaseUrl: "${PG_URL}"
clickhouse:
  url: "${CH_URL}"
  user: "${CH_USER}"
  db: "${CH_DB}"
  password: "${CH_PASSWORD}"
EOF
  else
    # ESO mode: no passwords in values, add secret store reference
    if [[ "$SECRETS_MODE" == "provider" ]]; then
      SECRET_ARN=$(jq -r '.secrets.provider.secret_arn // empty' "$HANDOFF")
      STORE_NAME="${PROJECT}-aws-sm"
      # Use ARN as the key — ESO resolves it for both TF-created and BYOA secrets
      SECRET_KEY="$SECRET_ARN"
    else
      STORE_NAME=$(jq -r '.secrets.external.store_name // empty' "$HANDOFF")
      SECRET_KEY=$(jq -r '.secrets.external.secret_key // empty' "$HANDOFF")
    fi
    STORE_KIND=$(jq -r '.secrets.external.store_kind // "ClusterSecretStore"' "$HANDOFF")

    cat >> "$OUT_FILE" <<EOF
secrets:
  storeName: "${STORE_NAME}"
  storeKind: "${STORE_KIND}"
  secretKey: "${SECRET_KEY}"
clickhouse:
  url: "${CH_URL}"
  user: "${CH_USER}"
  db: "${CH_DB}"
EOF
  fi

  # Inject nodeSelector if instance has a node_role
  if [[ -n "$NODE_ROLE" && "$NODE_ROLE" != "null" ]]; then
    cat >> "$OUT_FILE" <<EOF
nodeSelector:
  evm-cloud/role: "${NODE_ROLE}"
EOF
  fi

  # Inject user-defined extra env vars from handoff
  EXTRA_ENV_JSON=$(jq -c '.services.indexer.extra_env // {}' "$HANDOFF")
  if [[ "$EXTRA_ENV_JSON" != "{}" && "$EXTRA_ENV_JSON" != "null" ]]; then
    echo "extraEnv:" >> "$OUT_FILE"
    echo "$EXTRA_ENV_JSON" | jq -r 'to_entries[] | "  \(.key): \(.value | @json)"' >> "$OUT_FILE"
  fi

  # Inject user-defined secret env vars from handoff (stored in K8s Secret, referenced via secretKeyRef)
  EXTRA_SECRET_ENV_JSON=$(jq -c '.services.indexer.extra_secret_env // {}' "$HANDOFF")
  if [[ "$EXTRA_SECRET_ENV_JSON" != "{}" && "$EXTRA_SECRET_ENV_JSON" != "null" ]]; then
    echo "extraSecretEnv:" >> "$OUT_FILE"
    echo "$EXTRA_SECRET_ENV_JSON" | jq -r 'to_entries[] | "  \(.key): \(.value | @json)"' >> "$OUT_FILE"
  fi

  echo "Wrote $OUT_FILE"
}

INSTANCES=$(jq -c '.services.indexer.instances // null' "$HANDOFF")

if [[ "$INSTANCES" != "null" && "$INSTANCES" != "[]" ]]; then
  # Multi-instance: generate a values file per instance
  for INSTANCE in $(echo "$INSTANCES" | jq -c '.[]'); do
    NAME=$(echo "$INSTANCE" | jq -r '.name')
    NODE_ROLE=$(echo "$INSTANCE" | jq -r '.node_role // empty')
    WORKLOAD_TYPE=$(echo "$INSTANCE" | jq -r '.workload_type // "deployment"')
    render_indexer_values "$NAME" "$NODE_ROLE" "$OUT_DIR/${NAME}-values.yaml" "$WORKLOAD_TYPE"
  done
else
  # Single instance (backward compat)
  render_indexer_values "indexer" "" "$OUT_DIR/indexer-values.yaml"
fi

echo "Wrote $OUT_DIR/rpc-proxy-values.yaml"
