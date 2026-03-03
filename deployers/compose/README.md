# Compose Deployer

SSH-based deployer for Docker Compose hosts (EC2, VPS, bare metal).

## Prerequisites

- `jq`, `scp`, `ssh` on your local machine
- Docker + Docker Compose installed on the remote host
- SSH access to the remote host

## Usage

### Initial deploy

```bash
# From handoff output
terraform output -json workload_handoff | ./deployers/compose/deploy.sh /dev/stdin \
  --config-dir ./config \
  --ssh-key ~/.ssh/id_ed25519

# With explicit host (bare metal / non-AWS)
./deployers/compose/deploy.sh handoff.json \
  --config-dir ./config \
  --ssh-key ~/.ssh/id_ed25519 \
  --host 10.0.1.50 \
  --user ubuntu
```

### Config update

Same command — the script is idempotent. Update your config files locally, then re-run:

```bash
terraform output -json workload_handoff | ./deployers/compose/deploy.sh /dev/stdin \
  --config-dir ./config \
  --ssh-key ~/.ssh/id_ed25519
```

Containers are force-recreated to pick up bind-mounted config changes.

## Config directory structure

```
config/
  docker-compose.yml # Required — Docker Compose service definitions
  erpc.yaml          # eRPC config (when rpc_proxy_enabled)
  rindexer.yaml      # rindexer config (when indexer_enabled)
  abis/              # ABI JSON files
    MyContract.json
```

> `docker-compose.yml` is uploaded to `/opt/evm-cloud/docker-compose.yml` on the remote host. All other files go under `/opt/evm-cloud/config/`.

## Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `<handoff.json>` | Yes | Terraform workload_handoff output (file or `/dev/stdin`) |
| `--config-dir` | Yes | Path to config directory |
| `--ssh-key` | Yes | Path to SSH private key |
| `--host` | No | Override SSH host (default: from handoff `runtime.ec2.public_ip`) |
| `--user` | No | Override SSH user (default: `ec2-user` for EC2, `root` otherwise) |
| `--port` | No | Override SSH port (default: 22) |

## What it does

1. Ensures remote directories exist (`/opt/evm-cloud/config/abis/`)
2. Uploads `docker-compose.yml` to `/opt/evm-cloud/`
3. Uploads config files (`erpc.yaml`, `rindexer.yaml`, `abis/`) to `/opt/evm-cloud/config/`
4. Runs `pull-secrets.sh` on the remote (if present) to generate `.env` from AWS Secrets Manager
5. Runs `docker compose up -d --remove-orphans --force-recreate`

## Contract

Reads `workload_handoff` v1 JSON output. Uses:

- `compute_engine` — for SSH user defaults
- `runtime.ec2.public_ip` — SSH target (unless `--host` override)
