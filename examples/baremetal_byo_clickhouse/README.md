# Bare Metal + Docker Compose + ClickHouse BYODB Example

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ Your VPS (Ubuntu/Debian/RHEL)                                │
│                                                              │
│  ┌─ /opt/evm-cloud/ ──────────────────────────────────────┐  │
│  │                                                         │  │
│  │  Docker Compose (bridge network: evm-cloud)             │  │
│  │                                                         │  │
│  │  ┌─────────────────┐    ┌──────────────────────┐        │  │
│  │  │ eRPC Proxy      │    │ rindexer Indexer     │        │  │
│  │  │ (container)     │    │ (container)          │        │  │
│  │  │                 │    │                      │        │  │
│  │  │ Port 4000       │    │ depends_on: erpc     │        │  │
│  │  │ mem_limit: 1g   │    │ mem_limit: 2g        │        │  │
│  │  └────────┬────────┘    └──────────┬───────────┘        │  │
│  │           │ http://erpc:4000       │                    │  │
│  │           └────────────────────────┘                    │  │
│  │                                                         │  │
│  │  Config: config/erpc.yaml, config/rindexer.yaml         │  │
│  │  ABIs:   config/abis/*.json                             │  │
│  │  Secrets: .env (SSH-delivered, chmod 600)                │  │
│  │  Logging: json-file (local disk, 50m x 5 files)         │  │
│  └─────────────────────────────────────────────────────────┘  │
│                                                              │
│  Firewall: user-managed (UFW/iptables/firewalld)             │
└─────────────────────────────────────────────────────────────┘

        External:
        ┌───────────────────┐         ┌──────────────────────┐
        │ eth.llamarpc.com  │         │ ClickHouse (BYODB)   │
        │ (upstream RPC)    │         │ clickhouse.example.com│
        └────────┬──────────┘         └──────────┬───────────┘
                 │                               │
                 ▼                               ▲
            eRPC proxy ──RPC_URL──▶ rindexer ────┘
```

## What Gets Deployed

**Host Setup** (runs once on first provision, idempotent)

- Docker Engine + Compose plugin (via `apt-get` or `dnf`, with OS detection)
- Directory structure: `/opt/evm-cloud/{config/abis,scripts}`
- Docker service enabled and started

**eRPC proxy (Docker container)**

- Container: `erpc` — `ghcr.io/erpc/erpc:latest`
- Port 4000 on Docker bridge network
- Config: bind-mounted `/opt/evm-cloud/config/erpc.yaml`
- Proxies Ethereum mainnet via `https://eth.llamarpc.com`
- Healthcheck: `wget -q --spider http://localhost:4000/`
- Memory limit: 1g (configurable via `bare_metal_rpc_proxy_mem_limit`)

**rindexer indexer (Docker container)**

- Container: `rindexer` — `ghcr.io/joshstevens19/rindexer:latest`
- Config: bind-mounted `/opt/evm-cloud/config/` (rindexer.yaml + abis/)
- Indexes USDT Transfer events on Ethereum mainnet (blocks 19M-19M+100)
- Storage backend: **ClickHouse (BYODB)** — connects to your external ClickHouse instance
- ClickHouse credentials from `.env` file (SSH-delivered)
- `depends_on: erpc` with `service_healthy` condition
- Memory limit: 2g (configurable via `bare_metal_indexer_mem_limit`)

**Logging**

- Docker `json-file` driver (local disk, 50m rotation x 5 files)
- View logs: `docker compose logs -f` on the VPS

## Differences from other examples

| | `minimal_BYO_clickhouse` | `baremetal_byo_clickhouse` | `eks_BYO_clickhouse` | `bare_metal_k3s_byo_clickhouse` |
|---|---|---|---|---|
| **Provider** | AWS | Bare Metal | AWS | Bare Metal |
| **Compute** | EC2 + Docker Compose | Docker Compose (SSH) | EKS (Kubernetes) | k3s (Kubernetes) |
| **Database** | External ClickHouse | External ClickHouse | External ClickHouse | External ClickHouse |
| **Config delivery** | cloud-init + bind mounts | SSH file provisioner | ConfigMap volume mount | ConfigMap volume mount |
| **DB credentials** | Secrets Manager → `.env` | SSH → `.env` | K8s Secret | K8s Secret |
| **Logging** | CloudWatch (awslogs) | json-file (local disk) | `kubectl logs` | `kubectl logs` |
| **Networking** | VPC + Security Groups | User-managed firewall | VPC + SGs + NetworkPolicy | Flannel + NetworkPolicy |

## Data Flow

```
eth.llamarpc.com
      │
  eRPC (port 4000)         ← erpc.yaml bind-mounted from /opt/evm-cloud/config/
      │ (RPC_URL=http://erpc:4000, Docker Compose network)
  rindexer                  ← rindexer.yaml + abis/ bind-mounted
      │ (CLICKHOUSE_URL)
  ClickHouse (external)     ← creds from .env (SSH-delivered)
```

## Prerequisites

- A VPS with SSH access (Ubuntu 20.04+, Debian 11+, RHEL 8+, Amazon Linux 2023)
- SSH key pair (the user must have passwordless sudo)
- A ClickHouse instance (managed like ClickHouse Cloud, or self-hosted)
- Terraform >= 1.14.6

## Usage

```bash
# 1) Move into this example
cd examples/baremetal_byo_clickhouse

# 2) Copy secrets template and fill in real values
cp secrets.auto.tfvars.example secrets.auto.tfvars
# Edit secrets.auto.tfvars:
# - bare_metal_host                 = "203.0.113.10"
# - bare_metal_ssh_private_key_path = "~/.ssh/id_ed25519"
# - indexer_clickhouse_url          = "https://your-clickhouse-host:8443"
# - indexer_clickhouse_password     = "your-password"

# 3) Edit bare_metal_clickhouse.tfvars for your setup
#    - bare_metal_ssh_user (default: ubuntu)
#    - Resource limits (rpc_proxy_mem_limit, indexer_mem_limit)

# 4) Customize config/erpc.yaml and config/rindexer.yaml for your indexing needs

# 5) Initialize Terraform
terraform init

# 6) Review plan
terraform plan -var-file=bare_metal_clickhouse.tfvars

# 7) Apply
terraform apply -var-file=bare_metal_clickhouse.tfvars

# 8) Verify on the VPS
ssh <user>@<VPS>
cd /opt/evm-cloud && docker compose ps
docker compose logs -f

# 9) (Optional) Destroy when done
terraform destroy -var-file=bare_metal_clickhouse.tfvars
```

Sensitive values (`bare_metal_host`, `bare_metal_ssh_private_key_path`, `indexer_clickhouse_url`, `indexer_clickhouse_password`) go in `secrets.auto.tfvars` which is gitignored and auto-loaded by Terraform.

## Config updates (post-deploy)

Edit `config/erpc.yaml` or `config/rindexer.yaml` locally, then re-apply:

```bash
terraform apply -var-file=bare_metal_clickhouse.tfvars
```

Terraform detects config changes via content hash and re-deploys automatically (uploads new files, runs `docker compose up -d`).

## Firewall rules

Open these ports on your VPS firewall:

| Port | Protocol | Purpose |
|------|----------|---------|
| 22 | TCP | SSH (Terraform provisioning + admin) |
| 443 | TCP | Optional: HTTPS ingress to eRPC (if you add a reverse proxy) |

eRPC listens on `:4000` inside Docker. To expose it externally, add a reverse proxy (Caddy, nginx) or map the port in the compose config.

## What evm-cloud manages vs what you manage

| evm-cloud manages | You manage |
|-------------------|------------|
| Docker + Compose installation | VPS provisioning (Hetzner, OVH, etc.) |
| Config file delivery via SSH | DNS records |
| Container orchestration | Firewall rules (UFW/iptables) |
| Idempotent re-deployment | TLS certificates |
| .env secret delivery (chmod 600) | OS updates / patching |
| Container restart policy | ClickHouse provisioning |
| | Backups |
| | Monitoring / alerting |

## Workload ownership mode

This example defaults to:

- `compute_engine = "docker_compose"`
- `workload_mode = "terraform"`

Behavior by mode:

- `terraform`: Docker Compose services are provisioned and managed via SSH by Terraform.
- `external`: Terraform emits `workload_handoff` only; no provisioner resources created.

In external mode, use the compose deployer with the `workload_handoff` output:

```bash
terraform output -json workload_handoff | ./../../deployers/compose/deploy.sh /dev/stdin \
  --config-dir ./config --ssh-key ~/.ssh/id_ed25519
```
