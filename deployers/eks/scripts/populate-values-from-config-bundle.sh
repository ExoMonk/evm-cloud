#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<EOF
Usage:
  $0 --values-dir <path> --config-dir <path>

Expected files:
  <values-dir>/rpc-proxy-values.yaml
  <values-dir>/indexer-values.yaml
  <config-dir>/erpc.yaml
  <config-dir>/rindexer.yaml
  <config-dir>/abis/*.json
EOF
}

VALUES_DIR=""
CONFIG_DIR=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --values-dir) VALUES_DIR="$2"; shift 2 ;;
    --config-dir) CONFIG_DIR="$2"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; usage; exit 1 ;;
  esac
done

if [[ -z "$VALUES_DIR" || -z "$CONFIG_DIR" ]]; then
  usage
  exit 1
fi

if [[ ! -f "$VALUES_DIR/rpc-proxy-values.yaml" ]]; then
  echo "missing file: $VALUES_DIR/rpc-proxy-values.yaml" >&2
  exit 1
fi
if [[ ! -f "$VALUES_DIR/indexer-values.yaml" ]]; then
  echo "missing file: $VALUES_DIR/indexer-values.yaml" >&2
  exit 1
fi
if [[ ! -f "$CONFIG_DIR/erpc.yaml" ]]; then
  echo "missing file: $CONFIG_DIR/erpc.yaml" >&2
  exit 1
fi
if [[ ! -f "$CONFIG_DIR/rindexer.yaml" ]]; then
  echo "missing file: $CONFIG_DIR/rindexer.yaml" >&2
  exit 1
fi
if [[ ! -d "$CONFIG_DIR/abis" ]]; then
  echo "missing directory: $CONFIG_DIR/abis" >&2
  exit 1
fi

python3 - "$VALUES_DIR" "$CONFIG_DIR" <<'PY'
from pathlib import Path
import re
import sys

values_dir = Path(sys.argv[1])
config_dir = Path(sys.argv[2])

rpc_values_file = values_dir / "rpc-proxy-values.yaml"
indexer_values_file = values_dir / "indexer-values.yaml"

erpc = (config_dir / "erpc.yaml").read_text()
rindexer = (config_dir / "rindexer.yaml").read_text()

# --- RPC proxy: replace placeholder block under erpcYaml ---
# Matches the "# paste erpc.yaml content here" marker and all indented lines after it
rpc_values = rpc_values_file.read_text()
rpc_values = re.sub(
    r"# paste erpc\.yaml content here\n(    .+\n?)*",
    erpc.replace("\n", "\n    ") + "\n",
    rpc_values,
)
rpc_values_file.write_text(rpc_values)

# --- Indexer: replace placeholder block under rindexerYaml ---
# Matches the "# paste rindexer.yaml content here" marker and all indented lines after it
indexer_values = indexer_values_file.read_text()
indexer_values = re.sub(
    r"# paste rindexer\.yaml content here\n(    .+\n?)*",
    rindexer.replace("\n", "\n    ") + "\n",
    indexer_values,
)

# --- ABIs: replace empty abis dict with actual ABI files ---
abi_files = sorted((config_dir / "abis").glob("*.json"))
if abi_files:
    abi_block = "\n".join(
        f"    {abi.name}: |-\n" + "\n".join("      " + line for line in abi.read_text().splitlines())
        for abi in abi_files
    )
else:
    abi_block = "    {}"

indexer_values = indexer_values.replace("  abis: {}", "  abis:\n" + abi_block)
indexer_values_file.write_text(indexer_values)
PY

echo "Populated config in $VALUES_DIR/rpc-proxy-values.yaml"
echo "Populated config in $VALUES_DIR/indexer-values.yaml"
