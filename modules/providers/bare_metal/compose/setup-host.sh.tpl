#!/bin/bash
# Idempotent host setup: installs Docker + Compose plugin, creates directory structure.
# Safe to re-run — all operations check existing state before acting.
set -euo pipefail

echo "[evm-cloud] Setting up host for project: ${project_name}"

# --- OS detection and package manager ---
install_docker() {
  if command -v docker &>/dev/null && docker compose version &>/dev/null; then
    echo "[evm-cloud] Docker + Compose already installed: $(docker --version)"
    return
  fi

  echo "[evm-cloud] Installing Docker via get.docker.com (official convenience script)..."
  curl -fsSL https://get.docker.com | sudo sh

  sudo systemctl enable docker
  sudo systemctl start docker
  echo "[evm-cloud] Docker installed: $(docker --version)"
  echo "[evm-cloud] Compose plugin: $(docker compose version)"
}

# --- Ensure current user can run docker ---
ensure_docker_group() {
  if groups | grep -q docker; then
    return
  fi
  if [ "$(id -u)" = "0" ]; then
    return
  fi
  sudo usermod -aG docker "$USER"
  echo "[evm-cloud] Added $USER to docker group (re-login may be needed for interactive use)."
}

# --- Create directory structure ---
create_dirs() {
  sudo mkdir -p /opt/evm-cloud/config/abis
  sudo mkdir -p /opt/evm-cloud/scripts
  sudo mkdir -p /opt/evm-cloud/certs
  sudo chown -R "$USER:$USER" /opt/evm-cloud
  echo "[evm-cloud] Directory structure ready at /opt/evm-cloud/"
}

install_docker
ensure_docker_group
create_dirs

echo "[evm-cloud] Host setup complete."
