#!/usr/bin/env bash
set -euo pipefail

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required" >&2
  exit 1
fi
if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required" >&2
  exit 1
fi

usage() {
  cat <<EOF
Usage:
  $0 --handoff-file <path> --service <rpc-proxy|indexer> --image <image> --out <path>

Environment variables used by templates:
  CONFIG_BUNDLE_HASH (required)
  RPC_URL (optional for indexer; auto-derived from handoff Cloud Map URL, then ECS task fallback)
  DATABASE_SECRET_ARN / CLICKHOUSE_PASSWORD_SECRET_ARN (optional by backend)
  CLICKHOUSE_URL / CLICKHOUSE_USER / CLICKHOUSE_DB (optional by backend)
EOF
}

HANDOFF_FILE=""
SERVICE=""
IMAGE=""
OUT_FILE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --handoff-file) HANDOFF_FILE="$2"; shift 2 ;;
    --service) SERVICE="$2"; shift 2 ;;
    --image) IMAGE="$2"; shift 2 ;;
    --out) OUT_FILE="$2"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; usage; exit 1 ;;
  esac
done

if [[ -z "$HANDOFF_FILE" || -z "$SERVICE" || -z "$IMAGE" || -z "$OUT_FILE" ]]; then
  usage
  exit 1
fi

derive_rpc_url_from_ecs() {
  local handoff_file="$1"
  local region="$2"

  local stable_internal_url cluster_arn rpc_service_name rpc_service_port task_arn eni_id private_ip

  stable_internal_url=$(jq -r '.services.rpc_proxy.discovery.internal_url // empty' "$handoff_file")
  if [[ -n "$stable_internal_url" ]]; then
    echo "$stable_internal_url"
    return 0
  fi

  cluster_arn=$(jq -r '.runtime.ecs.cluster_arn // empty' "$handoff_file")
  rpc_service_name=$(jq -r '.services.rpc_proxy.service_name // empty' "$handoff_file")
  rpc_service_port=$(jq -r '.services.rpc_proxy.port // 4000' "$handoff_file")

  if [[ -z "$cluster_arn" || -z "$rpc_service_name" ]]; then
    echo "Cannot auto-derive RPC_URL: missing runtime.ecs.cluster_arn or services.rpc_proxy.service_name" >&2
    return 1
  fi

  task_arn=$(aws ecs list-tasks \
    --region "$region" \
    --cluster "$cluster_arn" \
    --service-name "$rpc_service_name" \
    --desired-status RUNNING \
    | jq -r '.taskArns[0] // empty')

  if [[ -z "$task_arn" ]]; then
    echo "Cannot auto-derive RPC_URL: no RUNNING task for rpc-proxy service '$rpc_service_name'" >&2
    return 1
  fi

  eni_id=$(aws ecs describe-tasks \
    --region "$region" \
    --cluster "$cluster_arn" \
    --tasks "$task_arn" \
    | jq -r '.tasks[0].attachments[]?.details | map(select(.name == "networkInterfaceId").value) | .[0] // empty')

  if [[ -z "$eni_id" ]]; then
    echo "Cannot auto-derive RPC_URL: could not resolve rpc-proxy ENI" >&2
    return 1
  fi

  private_ip=$(aws ec2 describe-network-interfaces \
    --region "$region" \
    --network-interface-ids "$eni_id" \
    | jq -r '.NetworkInterfaces[0].PrivateIpAddress // empty')

  if [[ -z "$private_ip" ]]; then
    echo "Cannot auto-derive RPC_URL: could not resolve rpc-proxy private IP" >&2
    return 1
  fi

  echo "http://${private_ip}:${rpc_service_port}"
}

MODE=$(jq -r '.mode' "$HANDOFF_FILE")
ENGINE=$(jq -r '.compute_engine' "$HANDOFF_FILE")
PROJECT=$(jq -r '.project_name' "$HANDOFF_FILE")
REGION=$(jq -r '.aws_region' "$HANDOFF_FILE")
BACKEND=$(jq -r '.data.backend // empty' "$HANDOFF_FILE")

if [[ "$MODE" != "external" || "$ENGINE" != "ecs" ]]; then
  echo "handoff must be mode=external and compute_engine=ecs" >&2
  exit 1
fi

if [[ -z "${CONFIG_BUNDLE_HASH:-}" ]]; then
  echo "CONFIG_BUNDLE_HASH is required" >&2
  exit 1
fi

BUCKET=$(jq -r '.artifacts.s3.bucket // empty' "$HANDOFF_FILE")
ERPC_KEY=$(jq -r '.artifacts.s3.erpc_config_key // empty' "$HANDOFF_FILE")
RINDEXER_KEY=$(jq -r '.artifacts.s3.rindexer_config_key // empty' "$HANDOFF_FILE")
RINDEXER_ABIS_PREFIX=$(jq -r '.artifacts.s3.rindexer_abis_prefix // empty' "$HANDOFF_FILE")
EXEC_ROLE=$(jq -r '.identity.ecs_task_execution_role_arn // empty' "$HANDOFF_FILE")
RPC_ROLE=$(jq -r '.identity.ecs_task_role_arns.rpc_proxy // empty' "$HANDOFF_FILE")
INDEXER_ROLE=$(jq -r '.identity.ecs_task_role_arns.indexer // empty' "$HANDOFF_FILE")

TEMPLATE_FILE=""
FAMILY=""
CPU=""
MEMORY=""
TASK_ROLE=""
LOG_GROUP=""

case "$SERVICE" in
  rpc-proxy)
    TEMPLATE_FILE="deployers/ecs/task-defs/rpc-proxy.taskdef.tpl.json"
    FAMILY="${PROJECT}-erpc"
    CPU="512"
    MEMORY="1024"
    TASK_ROLE="$RPC_ROLE"
    LOG_GROUP="/ecs/${PROJECT}/external/erpc"
    ERPC_CONFIG_S3_URI="s3://${BUCKET}/${ERPC_KEY}"
    if [[ -z "$BUCKET" || -z "$ERPC_KEY" ]]; then
      echo "Missing S3 config artifacts for rpc-proxy in workload_handoff.artifacts.s3" >&2
      exit 1
    fi
    ;;
  indexer)
    TEMPLATE_FILE="deployers/ecs/task-defs/indexer.taskdef.tpl.json"
    FAMILY="${PROJECT}-indexer"
    CPU="1024"
    MEMORY="2048"
    TASK_ROLE="$INDEXER_ROLE"
    LOG_GROUP="/ecs/${PROJECT}/external/indexer"
    RINDEXER_CONFIG_S3_URI="s3://${BUCKET}/${RINDEXER_KEY}"
    RINDEXER_ABIS_S3_URI="s3://${BUCKET}/${RINDEXER_ABIS_PREFIX}"
    if [[ -z "$BUCKET" || -z "$RINDEXER_KEY" || -z "$RINDEXER_ABIS_PREFIX" ]]; then
      echo "Missing S3 config artifacts for indexer in workload_handoff.artifacts.s3" >&2
      exit 1
    fi
    if [[ -z "${RPC_URL:-}" ]]; then
      RPC_URL="$(derive_rpc_url_from_ecs "$HANDOFF_FILE" "$REGION")" || exit 1
      echo "Auto-derived RPC_URL from rpc-proxy service: ${RPC_URL}" >&2
    fi
    ;;
  *)
    echo "service must be rpc-proxy or indexer" >&2
    exit 1
    ;;
esac

if [[ -z "$EXEC_ROLE" || -z "$TASK_ROLE" ]]; then
  echo "Missing ECS role ARNs in handoff.identity" >&2
  exit 1
fi

if [[ "$SERVICE" == "indexer" ]]; then
  if [[ "$BACKEND" == "postgres" ]]; then
    DATABASE_SECRET_ARN="${DATABASE_SECRET_ARN:-$(jq -r '.data.postgres.secret_arn // empty' "$HANDOFF_FILE")}"
    if [[ -z "$DATABASE_SECRET_ARN" ]]; then
      echo "DATABASE_SECRET_ARN missing for postgres backend" >&2
      exit 1
    fi
  elif [[ "$BACKEND" == "clickhouse" ]]; then
    CLICKHOUSE_URL="${CLICKHOUSE_URL:-$(jq -r '.data.clickhouse.url // empty' "$HANDOFF_FILE")}"
    CLICKHOUSE_USER="${CLICKHOUSE_USER:-$(jq -r '.data.clickhouse.user // "default"' "$HANDOFF_FILE")}"
    CLICKHOUSE_DB="${CLICKHOUSE_DB:-$(jq -r '.data.clickhouse.db // "default"' "$HANDOFF_FILE")}"
    if [[ -z "$CLICKHOUSE_URL" ]]; then
      echo "CLICKHOUSE_URL missing for clickhouse backend" >&2
      exit 1
    fi
    if [[ -z "${CLICKHOUSE_PASSWORD_SECRET_ARN:-}" ]]; then
      echo "CLICKHOUSE_PASSWORD_SECRET_ARN is required for clickhouse backend" >&2
      exit 1
    fi
  else
    echo "Unsupported or missing handoff data.backend: '$BACKEND'" >&2
    exit 1
  fi
fi

export FAMILY CPU MEMORY EXECUTION_ROLE_ARN="$EXEC_ROLE" TASK_ROLE_ARN="$TASK_ROLE" IMAGE AWS_REGION="$REGION" LOG_GROUP CONFIG_BUNDLE_HASH
export ERPC_CONFIG_S3_URI="${ERPC_CONFIG_S3_URI:-}" RINDEXER_CONFIG_S3_URI="${RINDEXER_CONFIG_S3_URI:-}" RINDEXER_ABIS_S3_URI="${RINDEXER_ABIS_S3_URI:-}"
export RPC_URL="${RPC_URL:-}" DATABASE_SECRET_ARN="${DATABASE_SECRET_ARN:-}" CLICKHOUSE_PASSWORD_SECRET_ARN="${CLICKHOUSE_PASSWORD_SECRET_ARN:-}"
export CLICKHOUSE_URL="${CLICKHOUSE_URL:-}" CLICKHOUSE_USER="${CLICKHOUSE_USER:-default}" CLICKHOUSE_DB="${CLICKHOUSE_DB:-default}"

python3 - "$TEMPLATE_FILE" "$OUT_FILE" <<'PY'
import os
import sys
from string import Template

src, dst = sys.argv[1], sys.argv[2]
with open(src, 'r', encoding='utf-8') as fh:
    raw = fh.read()
rendered = Template(raw).safe_substitute(os.environ)
with open(dst, 'w', encoding='utf-8') as fh:
    fh.write(rendered)
PY

jq . "$OUT_FILE" > /dev/null

if [[ "$SERVICE" == "indexer" ]]; then
  if [[ "$BACKEND" == "postgres" ]]; then
    jq --arg db_secret "$DATABASE_SECRET_ARN" '
      .containerDefinitions |= map(
        if .name == "indexer" then
          .environment = ((.environment // []) | map(select((.name | startswith("CLICKHOUSE_")) | not)))
          | .secrets = [
              {
                name: "DATABASE_URL",
                valueFrom: $db_secret
              }
            ]
        else
          .
        end
      )
    ' "$OUT_FILE" > "$OUT_FILE.tmp" && mv "$OUT_FILE.tmp" "$OUT_FILE"
  else
    jq --arg ch_secret "$CLICKHOUSE_PASSWORD_SECRET_ARN" '
      .containerDefinitions |= map(
        if .name == "indexer" then
          .secrets = [
            {
              name: "CLICKHOUSE_PASSWORD",
              valueFrom: $ch_secret
            }
          ]
        else
          .
        end
      )
    ' "$OUT_FILE" > "$OUT_FILE.tmp" && mv "$OUT_FILE.tmp" "$OUT_FILE"
  fi
fi

jq . "$OUT_FILE" > /dev/null

echo "Rendered $SERVICE task definition to $OUT_FILE"
