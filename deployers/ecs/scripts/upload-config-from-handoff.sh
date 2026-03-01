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
  $0 --handoff-file <path> --config-dir <path>

Uploads:
  <config-dir>/erpc.yaml      -> s3://<bucket>/<erpc_config_key>
  <config-dir>/rindexer.yaml  -> s3://<bucket>/<rindexer_config_key>
  <config-dir>/abis/*         -> s3://<bucket>/<rindexer_abis_prefix>
EOF
}

HANDOFF_FILE=""
CONFIG_DIR=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --handoff-file) HANDOFF_FILE="$2"; shift 2 ;;
    --config-dir) CONFIG_DIR="$2"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; usage; exit 1 ;;
  esac
done

if [[ -z "$HANDOFF_FILE" || -z "$CONFIG_DIR" ]]; then
  usage
  exit 1
fi

if [[ ! -f "$HANDOFF_FILE" ]]; then
  echo "handoff file not found: $HANDOFF_FILE" >&2
  exit 1
fi

ERPC_FILE="$CONFIG_DIR/erpc.yaml"
RINDEXER_FILE="$CONFIG_DIR/rindexer.yaml"
ABIS_DIR="$CONFIG_DIR/abis"

if [[ ! -f "$ERPC_FILE" ]]; then
  echo "missing file: $ERPC_FILE" >&2
  exit 1
fi
if [[ ! -f "$RINDEXER_FILE" ]]; then
  echo "missing file: $RINDEXER_FILE" >&2
  exit 1
fi
if [[ ! -d "$ABIS_DIR" ]]; then
  echo "missing directory: $ABIS_DIR" >&2
  exit 1
fi

BUCKET=$(jq -r '.artifacts.s3.bucket // empty' "$HANDOFF_FILE")
ERPC_KEY=$(jq -r '.artifacts.s3.erpc_config_key // empty' "$HANDOFF_FILE")
RINDEXER_KEY=$(jq -r '.artifacts.s3.rindexer_config_key // empty' "$HANDOFF_FILE")
RINDEXER_ABIS_PREFIX=$(jq -r '.artifacts.s3.rindexer_abis_prefix // empty' "$HANDOFF_FILE")

if [[ -z "$BUCKET" || -z "$ERPC_KEY" || -z "$RINDEXER_KEY" || -z "$RINDEXER_ABIS_PREFIX" ]]; then
  echo "Missing required artifacts.s3 keys in workload_handoff" >&2
  exit 1
fi

aws s3 cp "$ERPC_FILE" "s3://$BUCKET/$ERPC_KEY"
aws s3 cp "$RINDEXER_FILE" "s3://$BUCKET/$RINDEXER_KEY"
aws s3 sync "$ABIS_DIR/" "s3://$BUCKET/$RINDEXER_ABIS_PREFIX"

echo "Uploaded config bundle to s3://$BUCKET"
