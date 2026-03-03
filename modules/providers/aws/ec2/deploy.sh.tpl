#!/bin/bash
# Idempotent deploy: refresh secrets, extract ABIs, restart containers.
# Safe to re-run — docker compose handles container lifecycle.
set -euo pipefail

echo "[evm-cloud] Deploying config update for: ${project_name}"

cd /opt/evm-cloud

# --- Refresh secrets from AWS Secrets Manager ---
# Runs as root because .env is owned by root from cloud-init
if [ -x scripts/pull-secrets.sh ]; then
  echo "[evm-cloud] Refreshing secrets from Secrets Manager..."
  sudo bash scripts/pull-secrets.sh
fi

# --- Extract ABI files from JSON manifest ---
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
sudo chmod 600 .env 2>/dev/null || true

# --- Docker Compose up ---
# --force-recreate ensures containers pick up changed bind-mounted configs
echo "[evm-cloud] Running docker compose up..."
sudo docker compose up -d --remove-orphans --force-recreate

echo "[evm-cloud] Deploy complete. Running containers:"
sudo docker compose ps
