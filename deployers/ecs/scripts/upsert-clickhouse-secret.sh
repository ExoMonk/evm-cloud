#!/usr/bin/env bash
set -euo pipefail

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required" >&2
  exit 1
fi
if ! command -v aws >/dev/null 2>&1; then
  echo "aws CLI is required" >&2
  exit 1
fi

usage() {
  cat <<EOF
Usage:
  $0 --handoff-file <path> --secret-value <value> [--secret-name <name>]

Defaults:
  --secret-name evm-cloud/indexer/clickhouse-password

Output:
  Prints secret ARN to stdout.
EOF
}

HANDOFF_FILE=""
SECRET_NAME="evm-cloud/indexer/clickhouse-password"
SECRET_VALUE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --handoff-file) HANDOFF_FILE="$2"; shift 2 ;;
    --secret-name) SECRET_NAME="$2"; shift 2 ;;
    --secret-value) SECRET_VALUE="$2"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; usage; exit 1 ;;
  esac
done

if [[ -z "$HANDOFF_FILE" || -z "$SECRET_VALUE" ]]; then
  usage
  exit 1
fi
if [[ -z "$SECRET_NAME" ]]; then
  echo "secret name cannot be empty" >&2
  exit 1
fi

if [[ ! -f "$HANDOFF_FILE" ]]; then
  echo "handoff file not found: $HANDOFF_FILE" >&2
  exit 1
fi

AWS_REGION="$(jq -r '.aws_region // empty' "$HANDOFF_FILE")"
if [[ -z "$AWS_REGION" ]]; then
  echo "Missing aws_region in workload_handoff" >&2
  exit 1
fi

CLICKHOUSE_SECRET_ARN=""
if ! CLICKHOUSE_SECRET_ARN=$(aws secretsmanager describe-secret \
  --secret-id "$SECRET_NAME" \
  --region "$AWS_REGION" \
  --query 'ARN' \
  --output text 2>/dev/null); then
  CREATE_OUTPUT=""
  if ! CREATE_OUTPUT=$(aws secretsmanager create-secret \
    --name "$SECRET_NAME" \
    --secret-string "$SECRET_VALUE" \
    --region "$AWS_REGION" \
    --output json 2>&1); then
    if [[ "$CREATE_OUTPUT" != *"ResourceExistsException"* ]]; then
      echo "$CREATE_OUTPUT" >&2
      exit 1
    fi
  fi

  CLICKHOUSE_SECRET_ARN=$(aws secretsmanager describe-secret \
    --secret-id "$SECRET_NAME" \
    --region "$AWS_REGION" \
    --query 'ARN' \
    --output text)
fi

if [[ -z "$CLICKHOUSE_SECRET_ARN" || "$CLICKHOUSE_SECRET_ARN" == "None" ]]; then
  echo "Could not resolve secret ARN for $SECRET_NAME in region $AWS_REGION" >&2
  exit 1
fi

aws secretsmanager put-secret-value \
  --secret-id "$CLICKHOUSE_SECRET_ARN" \
  --secret-string "$SECRET_VALUE" \
  --region "$AWS_REGION" >/dev/null

echo "$CLICKHOUSE_SECRET_ARN"
