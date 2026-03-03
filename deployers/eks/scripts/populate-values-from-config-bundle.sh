#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<EOF
Usage:
  $0 --values-dir <path> --config-dir <path>

Expected files:
  <values-dir>/rpc-proxy-values.yaml
  <values-dir>/indexer-values.yaml   (or per-instance: <name>-values.yaml)
  <config-dir>/erpc.yaml
  <config-dir>/rindexer.yaml         (default indexer config)
  <config-dir>/<config_key>/rindexer.yaml  (optional per-instance override)
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

# Find all indexer values files (indexer-values.yaml or <name>-values.yaml)
# Excludes rpc-proxy-values.yaml
INDEXER_VALUES_FILES=()
for f in "$VALUES_DIR"/*-values.yaml; do
  [[ "$(basename "$f")" == "rpc-proxy-values.yaml" ]] && continue
  INDEXER_VALUES_FILES+=("$f")
done

if [[ ${#INDEXER_VALUES_FILES[@]} -eq 0 ]]; then
  echo "ERROR: No indexer values files found in $VALUES_DIR" >&2
  exit 1
fi

python3 - "$VALUES_DIR" "$CONFIG_DIR" "${INDEXER_VALUES_FILES[@]}" <<'PY'
from pathlib import Path
import re
import sys

values_dir = Path(sys.argv[1])
config_dir = Path(sys.argv[2])
indexer_values_files = [Path(f) for f in sys.argv[3:]]

# --- RPC proxy: replace placeholder block under erpcYaml ---
erpc = (config_dir / "erpc.yaml").read_text()
rpc_values_file = values_dir / "rpc-proxy-values.yaml"
rpc_values = rpc_values_file.read_text()
rpc_values = re.sub(
    r"# paste erpc\.yaml content here\n(    .+\n?)*",
    erpc.replace("\n", "\n    ") + "\n",
    rpc_values,
)
rpc_values_file.write_text(rpc_values)
print(f"Populated config in {rpc_values_file}")

# --- Read ABIs (shared across all instances) ---
abi_dir = config_dir / "abis"
abi_files = sorted(abi_dir.glob("*.json"))
if abi_files:
    abi_block = "\n".join(
        f"    {abi.name}: |-\n" + "\n".join("      " + line for line in abi.read_text().splitlines())
        for abi in abi_files
    )
else:
    abi_block = "    {}"

# --- Indexer: populate each values file with its config ---
default_rindexer = (config_dir / "rindexer.yaml").read_text()

for ivf in indexer_values_files:
    # Derive config_key from filename: "backfill-values.yaml" -> "backfill"
    # "indexer-values.yaml" -> "indexer" (uses default config)
    stem = ivf.name.replace("-values.yaml", "")

    # Look for per-instance config: config/<config_key>/rindexer.yaml
    instance_config = config_dir / stem / "rindexer.yaml"
    if instance_config.exists():
        rindexer = instance_config.read_text()
        print(f"  {ivf.name}: using instance config from {instance_config}")
    else:
        rindexer = default_rindexer
        print(f"  {ivf.name}: using default config")

    indexer_values = ivf.read_text()
    indexer_values = re.sub(
        r"# paste rindexer\.yaml content here\n(    .+\n?)*",
        rindexer.replace("\n", "\n    ") + "\n",
        indexer_values,
    )
    indexer_values = indexer_values.replace("  abis: {}", "  abis:\n" + abi_block)
    ivf.write_text(indexer_values)
    print(f"Populated config in {ivf}")
PY
