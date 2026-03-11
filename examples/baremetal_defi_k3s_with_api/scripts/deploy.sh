#!/usr/bin/env bash
# Full deployment pipeline: build swap-api → ship to VPS → provision infra → deploy workloads
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
API_DIR="$ROOT_DIR/api"
IMAGE_NAME="docker.io/library/swap-api"
IMAGE_TAG="${IMAGE_TAG:-local}"
IMAGE_TAR="/tmp/swap-api.tar"

# ---------------------------------------------------------------------------
# Read SSH config from tfvars
# ---------------------------------------------------------------------------
read_tfvar() {
  local key="$1"
  local file="$ROOT_DIR/secrets.auto.tfvars"
  if [[ ! -f "$file" ]]; then
    echo "ERROR: $file not found. Copy secrets.auto.tfvars.example and fill in values." >&2
    exit 1
  fi
  grep -E "^${key}\s*=" "$file" | sed 's/[^=]*=[ ]*//' | sed 's/^"//; s/".*//' | sed 's/[ ]*#.*//' | sed 's/^[ ]*//; s/[ ]*$//' | head -1
}

VPS_HOST="${VPS_HOST:-$(read_tfvar bare_metal_host)}"
VPS_USER="${VPS_USER:-$(read_tfvar bare_metal_ssh_user || echo ubuntu)}"
SSH_KEY="${SSH_KEY:-$(read_tfvar ssh_private_key_path)}"
SSH_PORT="${SSH_PORT:-$(read_tfvar bare_metal_ssh_port || echo 22)}"

# Expand ~ in SSH_KEY
SSH_KEY="${SSH_KEY/#\~/$HOME}"

SSH_ARGS=(-i "$SSH_KEY" -p "$SSH_PORT" -o StrictHostKeyChecking=no -o ConnectTimeout=10)
SCP_ARGS=(-i "$SSH_KEY" -P "$SSH_PORT" -o StrictHostKeyChecking=no -o ConnectTimeout=10)

echo "=== Configuration ==="
echo "  VPS:   ${VPS_USER}@${VPS_HOST}:${SSH_PORT}"
echo "  Image: ${IMAGE_NAME}:${IMAGE_TAG}"
echo ""

# ---------------------------------------------------------------------------
# Step 0: Initialize project (creates minimal evm-cloud.toml if needed)
# ---------------------------------------------------------------------------
echo "=== Step 0: Initializing project ==="
cd "$ROOT_DIR"
if [[ ! -f "evm-cloud.toml" ]]; then
  evm-cloud init --non-interactive
  echo "  ✓ Project initialized"
else
  echo "  ✓ Already initialized"
fi

# ---------------------------------------------------------------------------
# Step 1: Build swap-api Docker image
# ---------------------------------------------------------------------------
echo ""
echo "=== Step 1: Building swap-api image ==="
docker build --platform linux/amd64 -t "${IMAGE_NAME}:${IMAGE_TAG}" "$API_DIR"
echo "  ✓ Image built"

# ---------------------------------------------------------------------------
# Step 2: Export and ship to VPS
# ---------------------------------------------------------------------------
echo ""
echo "=== Step 2: Shipping image to VPS ==="
docker save "${IMAGE_NAME}:${IMAGE_TAG}" -o "$IMAGE_TAR"
echo "  ✓ Saved to $IMAGE_TAR ($(du -h "$IMAGE_TAR" | cut -f1))"

scp "${SCP_ARGS[@]}" "$IMAGE_TAR" "${VPS_USER}@${VPS_HOST}:/tmp/swap-api.tar"
echo "  ✓ Uploaded to VPS"

ssh "${SSH_ARGS[@]}" "${VPS_USER}@${VPS_HOST}" "sudo k3s ctr images import /tmp/swap-api.tar && rm /tmp/swap-api.tar"
echo "  ✓ Imported into k3s containerd"

rm -f "$IMAGE_TAR"

# ---------------------------------------------------------------------------
# Step 3: Terraform apply (provision k3s on VPS)
# ---------------------------------------------------------------------------
echo ""
echo "=== Step 3: Provisioning infrastructure ==="
cd "$ROOT_DIR"
evm-cloud apply
echo "  ✓ Infrastructure provisioned"

# ---------------------------------------------------------------------------
# Step 4: Deploy workloads (eRPC + rindexer + swap-api)
# ---------------------------------------------------------------------------
echo ""
echo "=== Step 4: Deploying workloads ==="
evm-cloud deploy
echo "  ✓ Workloads deployed"

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------
echo ""
echo "=== Deployment complete ==="
echo ""
echo "  Verify:"
echo "    evm-cloud status"
echo "    evm-cloud logs swap-api"
echo "    evm-cloud logs indexer"
echo ""
echo "  Query the API:"
echo "    kubectl port-forward -n evm-cloud svc/defi-swaps-swap-api 3000:3000 &"
echo "    curl http://localhost:3000/health"
echo "    curl 'http://localhost:3000/swaps?limit=5&network=base'"
echo "    curl http://localhost:3000/stats"
echo "    curl http://localhost:3000/alerts"
