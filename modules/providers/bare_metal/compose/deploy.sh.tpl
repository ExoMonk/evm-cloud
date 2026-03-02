#!/bin/bash
# Idempotent deploy: extract ABI files from manifest, run docker compose up.
# Safe to re-run — docker compose handles container lifecycle.
set -euo pipefail

echo "[evm-cloud] Deploying project: ${project_name}"

cd /opt/evm-cloud

# --- Extract ABI files from JSON manifest ---
# The manifest is a JSON object { "filename.json": "content", ... }
# We extract each key as a file in config/abis/
if [ -f config/abis/_manifest.json ] && command -v python3 &>/dev/null; then
  python3 -c "
import json, os
with open('config/abis/_manifest.json') as f:
    abis = json.load(f)
for name, content in abis.items():
    path = os.path.join('config/abis', name)
    with open(path, 'w') as out:
        out.write(content)
    print(f'[evm-cloud] Wrote ABI: {path}')
" 2>/dev/null || echo "[evm-cloud] No python3 — skipping ABI extraction"
elif [ -f config/abis/_manifest.json ]; then
  echo "[evm-cloud] WARNING: python3 not found, ABI files not extracted from manifest."
fi

# --- Secure .env permissions ---
chmod 600 .env 2>/dev/null || true

# --- Docker Compose up ---
# --force-recreate ensures containers pick up changed bind-mounted configs
# (docker compose only detects compose.yml/env changes, not mounted file changes)
echo "[evm-cloud] Running docker compose up..."
docker compose up -d --remove-orphans --force-recreate

echo "[evm-cloud] Deploy complete. Running containers:"
docker compose ps
