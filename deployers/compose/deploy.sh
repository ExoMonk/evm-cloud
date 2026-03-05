#!/usr/bin/env bash
# Compose Deployer — deploys workloads to any Docker Compose host via SSH.
# Reads from Terraform workload_handoff output (JSON) + config directory.
# Works for AWS EC2, bare-metal VPS, or any host with Docker Compose.
#
# Usage:
#   terraform output -json workload_handoff | ./deployers/compose/deploy.sh /dev/stdin --config-dir ./config --ssh-key ~/.ssh/id_ed25519
#   # or
#   ./deployers/compose/deploy.sh handoff.json --config-dir ./config --ssh-key ~/.ssh/id_ed25519
#   # Override SSH user/host:
#   ./deployers/compose/deploy.sh handoff.json --config-dir ./config --ssh-key ~/.ssh/id_ed25519 --host 10.0.1.50 --user ubuntu
set -euo pipefail

# --- Parse arguments ---

HANDOFF=""
CONFIG_DIR=""
SSH_KEY=""
SSH_USER=""
SSH_HOST=""
SSH_PORT=22

while [[ $# -gt 0 ]]; do
  case "$1" in
    --config-dir) CONFIG_DIR="$2"; shift 2 ;;
    --ssh-key)    SSH_KEY="$2"; shift 2 ;;
    --user)       SSH_USER="$2"; shift 2 ;;
    --host)       SSH_HOST="$2"; shift 2 ;;
    --port)       SSH_PORT="$2"; shift 2 ;;
    *)            HANDOFF="$1"; shift ;;
  esac
done

if [[ -z "$HANDOFF" ]]; then
  echo "Usage: $0 <handoff.json> --config-dir <path> --ssh-key <path> [--host <ip>] [--user <name>] [--port <num>]" >&2
  echo "  terraform output -json workload_handoff | $0 /dev/stdin --config-dir ./config --ssh-key ~/.ssh/id_ed25519" >&2
  exit 1
fi

if [[ -z "$CONFIG_DIR" ]]; then
  echo "ERROR: --config-dir is required (path to docker-compose.yml, erpc.yaml, rindexer.yaml, abis/)" >&2
  exit 1
fi

if [[ -z "$SSH_KEY" ]]; then
  echo "ERROR: --ssh-key is required (path to SSH private key)" >&2
  exit 1
fi

for cmd in jq scp ssh; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "ERROR: $cmd is required but not found in PATH" >&2
    exit 1
  fi
done

# --- Buffer handoff ---

HANDOFF_FILE=$(mktemp /tmp/compose-handoff.XXXXXX)
trap "rm -f '$HANDOFF_FILE'" EXIT

cat "$HANDOFF" > "$HANDOFF_FILE"
chmod 0600 "$HANDOFF_FILE"

# --- Parse handoff ---

ENGINE=$(jq -r '.compute_engine' "$HANDOFF_FILE")
PROJECT=$(jq -r '.project_name' "$HANDOFF_FILE")
RPC_PROXY_ENABLED=$(jq -r '.services.rpc_proxy != null' "$HANDOFF_FILE")
INDEXER_ENABLED=$(jq -r '.services.indexer != null' "$HANDOFF_FILE")

# Resolve SSH host from handoff or CLI override
if [[ -z "$SSH_HOST" ]]; then
  SSH_HOST=$(jq -r '.runtime.ec2.public_ip // .runtime.bare_metal.host_address // empty' "$HANDOFF_FILE")
fi
if [[ -z "$SSH_HOST" ]]; then
  echo "ERROR: No SSH host found. Provide --host or ensure handoff contains runtime.ec2.public_ip or runtime.bare_metal.host_address." >&2
  exit 1
fi

# Resolve SSH user: CLI override > default per compute engine
if [[ -z "$SSH_USER" ]]; then
  if [[ "$ENGINE" == "ec2" ]]; then
    SSH_USER="ec2-user"
  else
    SSH_USER="root"
  fi
fi

REMOTE_DIR="/opt/evm-cloud"
SSH_COMMON="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -i ${SSH_KEY}"
SSH_OPTS="${SSH_COMMON} -p ${SSH_PORT}"
SCP_OPTS="${SSH_COMMON} -P ${SSH_PORT}"

echo "[evm-cloud] Deploying to ${SSH_USER}@${SSH_HOST}:${SSH_PORT} (project: ${PROJECT})"

# --- Verify connectivity ---

if ! ssh $SSH_OPTS "${SSH_USER}@${SSH_HOST}" "echo ok" >/dev/null 2>&1; then
  echo "ERROR: Cannot SSH to ${SSH_USER}@${SSH_HOST}:${SSH_PORT}" >&2
  exit 1
fi
echo "[evm-cloud] SSH connectivity verified."

# --- Ensure remote directories exist ---

ssh $SSH_OPTS "${SSH_USER}@${SSH_HOST}" "sudo mkdir -p ${REMOTE_DIR}/config/abis && sudo chown -R \$(whoami) ${REMOTE_DIR}"

# --- Upload all configs ---

if [[ ! -f "${CONFIG_DIR}/docker-compose.yml" ]]; then
  echo "ERROR: ${CONFIG_DIR}/docker-compose.yml not found. External mode requires a compose file in your config directory." >&2
  exit 1
fi

echo "[evm-cloud] Uploading config directory to ${REMOTE_DIR}/..."

# docker-compose.yml goes to the root, everything else under config/
scp $SCP_OPTS "${CONFIG_DIR}/docker-compose.yml" "${SSH_USER}@${SSH_HOST}:${REMOTE_DIR}/docker-compose.yml"
scp -r $SCP_OPTS "${CONFIG_DIR}/erpc.yaml" "${CONFIG_DIR}/rindexer.yaml" "${SSH_USER}@${SSH_HOST}:${REMOTE_DIR}/config/" 2>/dev/null || true
if [[ -d "${CONFIG_DIR}/abis" ]]; then
  scp -r $SCP_OPTS "${CONFIG_DIR}/abis" "${SSH_USER}@${SSH_HOST}:${REMOTE_DIR}/config/"
fi

echo "[evm-cloud] Uploaded configs."

# --- Deploy .env (secrets) ---

if [[ -f "${CONFIG_DIR}/.env" ]]; then
  # CLI generated .env from tfvars secrets — upload it directly.
  echo "[evm-cloud] Uploading .env from config directory..."
  scp $SCP_OPTS "${CONFIG_DIR}/.env" "${SSH_USER}@${SSH_HOST}:${REMOTE_DIR}/.env"
  ssh $SSH_OPTS "${SSH_USER}@${SSH_HOST}" "chmod 0600 ${REMOTE_DIR}/.env"
  echo "[evm-cloud] Uploaded .env"
elif ssh $SSH_OPTS "${SSH_USER}@${SSH_HOST}" "test -x ${REMOTE_DIR}/scripts/pull-secrets.sh" 2>/dev/null; then
  # AWS path: pull from Secrets Manager.
  echo "[evm-cloud] Pulling secrets from Secrets Manager..."
  ssh $SSH_OPTS "${SSH_USER}@${SSH_HOST}" "cd ${REMOTE_DIR} && bash scripts/pull-secrets.sh"
  echo "[evm-cloud] Secrets pulled to .env"
else
  # Ensure .env exists (docker compose requires it via env_file directive).
  ssh $SSH_OPTS "${SSH_USER}@${SSH_HOST}" "touch ${REMOTE_DIR}/.env"
fi

# --- Restart containers ---

echo "[evm-cloud] Restarting containers..."
ssh $SSH_OPTS "${SSH_USER}@${SSH_HOST}" "cd ${REMOTE_DIR} && docker compose up -d --remove-orphans --force-recreate"

echo "[evm-cloud] Verifying containers..."
ssh $SSH_OPTS "${SSH_USER}@${SSH_HOST}" "cd ${REMOTE_DIR} && docker compose ps"

echo "[evm-cloud] Deploy complete."
