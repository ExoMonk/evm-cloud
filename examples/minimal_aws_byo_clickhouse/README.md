# Minimal ClickHouse BYODB Example

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│ AWS (us-east-1)                                                     │
│                                                                     │
│  ┌──────────────────────────── VPC (10.42.0.0/16) ───────────────┐  │
│  │                                                                │  │
│  │  ┌─ Public Subnets ───────────────────────────────────────┐   │  │
│  │  │                                                         │   │  │
│  │  │  ┌─ EC2 Instance (t3.micro) ─────────────────────────┐  │   │  │
│  │  │  │                                                    │  │   │  │
│  │  │  │  Docker Compose (bridge network: evm-cloud)        │  │   │  │
│  │  │  │                                                    │  │   │  │
│  │  │  │  ┌─────────────────┐    ┌──────────────────────┐   │  │   │  │
│  │  │  │  │ eRPC Proxy      │    │ rindexer Indexer     │   │  │   │  │
│  │  │  │  │ (container)     │    │ (container)          │   │  │   │  │
│  │  │  │  │                 │    │                      │   │  │   │  │
│  │  │  │  │ Port 4000       │    │ depends_on: erpc     │   │  │   │  │
│  │  │  │  │ mem_limit: 256m  │    │ mem_limit: 512m       │   │  │   │  │
│  │  │  │  └────────┬────────┘    └──────────┬───────────┘   │  │   │  │
│  │  │  │           │ http://erpc:4000       │               │  │   │  │
│  │  │  │           └────────────────────────┘               │  │   │  │
│  │  │  │                                                    │  │   │  │
│  │  │  │  Config: /opt/evm-cloud/config/                    │  │   │  │
│  │  │  │    erpc.yaml, rindexer.yaml, abis/*.json           │  │   │  │
│  │  │  │  Secrets: /opt/evm-cloud/.env (from SM)            │  │   │  │
│  │  │  └────────────────────────────────────────────────────┘  │   │  │
│  │  │                                                         │   │  │
│  │  │  Internet Gateway                                       │   │  │
│  │  └─────────────────────────────────────────────────────────┘   │  │
│  │                                                                │  │
│  │  ┌─ IAM ──────────────────┐  ┌─ CloudWatch ────────────────┐  │  │
│  │  │ EC2 instance role       │  │ /evm-cloud/...  (30d)       │  │  │
│  │  │  logs:PutLogEvents      │  └─────────────────────────────┘  │  │
│  │  │  sm:GetSecretValue      │                                   │  │
│  │  └─────────────────────────┘  ┌─ Secrets Manager ───────────┐  │  │
│  │                                │ CLICKHOUSE_URL/USER/PASS/DB │  │  │
│  │                                │ RPC_URL                     │  │  │
│  │                                └─────────────────────────────┘  │  │
│  └────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘

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

### Networking (VPC layer)

- VPC (`10.42.0.0/16`) with DNS support
- 2 public subnets + 2 private subnets across `us-east-1a` and `us-east-1b`
- Internet Gateway for public subnet egress
- Route tables for public/private routing
- 3 Security Groups: erpc (port 4000), indexer (egress-only), bastion

### IAM

- EC2 instance role with policies:
  - `logs:CreateLogStream` + `logs:PutLogEvents` on CloudWatch log group
  - `secretsmanager:GetSecretValue` on `evm-cloud/<project>/*`
- IAM instance profile attached to the EC2 instance

### Compute (EC2 + Docker Compose)

- Single EC2 instance (`t3.micro` default — free tier eligible, configurable via `ec2_instance_type`)
- Amazon Linux 2023 AMI
- SSH key pair from `ssh_public_key` variable
- 30GB gp3 encrypted root volume
- cloud-init bootstraps: Docker + Compose plugin, config files, secrets pull, `docker compose up -d`

### eRPC proxy (Docker container)

- Container: `erpc` — `ghcr.io/erpc/erpc:latest`
- Port 4000 exposed on host
- Config: bind-mounted `/opt/evm-cloud/config/erpc.yaml`
- Proxies Ethereum mainnet via `https://eth.llamarpc.com` with retry (5x) + hedge (2x)
- Healthcheck: `wget -q --spider http://localhost:4000/`
- Memory limit: 256m (configurable via `ec2_rpc_proxy_mem_limit`)

### rindexer indexer (Docker container)

- Container: `rindexer` — `ghcr.io/joshstevens19/rindexer:latest`
- Config: bind-mounted `/opt/evm-cloud/config/` (rindexer.yaml + abis/)
- Indexes USDT Transfer events on Ethereum mainnet (blocks 19M-19M+100)
- Storage backend: **ClickHouse (BYODB)** — connects to your external ClickHouse instance
- ClickHouse credentials from Secrets Manager → `.env` file
- `depends_on: erpc` with `service_healthy` condition
- Memory limit: 512m (configurable via `ec2_indexer_mem_limit`)

### Logging

- CloudWatch Log Group: `/evm-cloud/<project>-<env>` (30-day retention)
- Docker `awslogs` driver ships container logs (streams: `erpc`, `rindexer`)

## Differences from other examples

| | `minimal_rds` | `minimal_BYO_clickhouse` | `eks_BYO_clickhouse` |
|---|---|---|---|
| **Compute** | EC2 + Docker Compose | EC2 + Docker Compose | EKS (Kubernetes) |
| **Database** | Managed RDS PostgreSQL | External ClickHouse | External ClickHouse |
| **Config delivery** | cloud-init + bind mounts | cloud-init + bind mounts | ConfigMap volume mount |
| **DB credentials** | Secrets Manager → `.env` | Secrets Manager → `.env` | K8s Secret |
| **Logging** | CloudWatch (awslogs) | CloudWatch (awslogs) | `kubectl logs` |

## Data Flow

```
eth.llamarpc.com
      │
  eRPC (port 4000)         ← erpc.yaml bind-mounted from /opt/evm-cloud/config/
      │ (RPC_URL=http://erpc:4000, Docker Compose network)
  rindexer                  ← rindexer.yaml + abis/ bind-mounted
      │ (CLICKHOUSE_URL)
  ClickHouse (external)     ← creds from Secrets Manager → .env
```

## Usage

```bash
# 1) Move into this example
cd examples/minimal_aws_byo_clickhouse

# 2) Copy secrets template and fill in real values
cp secrets.auto.tfvars.example secrets.auto.tfvars
# Edit secrets.auto.tfvars:
# - indexer_clickhouse_password = "your-password"
# - ssh_public_key              = "ssh-ed25519 AAAA..."

# 3) Initialize Terraform
terraform init

# 4) Review plan
terraform plan -var-file=minimal_clickhouse.tfvars

# 5) Apply
terraform apply -var-file=minimal_clickhouse.tfvars

# 6) SSH into the instance
ssh -i ~/.ssh/your-key ec2-user@<public-ip>

# 7) Check running containers
sudo docker compose -f /opt/evm-cloud/docker-compose.yml ps

# 8) View logs
sudo docker compose -f /opt/evm-cloud/docker-compose.yml logs -f

# 9) (Optional) Destroy when done
terraform destroy -var-file=minimal_clickhouse.tfvars
```

Sensitive values (`indexer_clickhouse_password`, `ssh_public_key`) go in `secrets.auto.tfvars` which is gitignored and auto-loaded by Terraform.

## Config updates (post-deploy)

Edit config files locally, then re-apply:

```bash
vim config/erpc.yaml
vim config/rindexer.yaml

terraform apply -var-file=minimal_clickhouse.tfvars
```

Terraform detects config changes via content hash and automatically pushes updated files to the EC2 instance via SSH, then force-recreates Docker Compose containers. The EC2 instance is **not** recreated — only configs and containers are updated.

> **Requires:** `ssh_private_key_path` must be set in `secrets.auto.tfvars` (path to the SSH private key matching `ssh_public_key`).

If using `workload_mode = "external"`, use the compose deployer instead:

```bash
terraform output -json workload_handoff | ./../../deployers/compose/deploy.sh /dev/stdin \
  --config-dir ./config --ssh-key ~/.ssh/id_ed25519
```

## Lifecycle behavior

- **Config changes** (erpc.yaml, rindexer.yaml, ABIs, mem limits): `lifecycle { ignore_changes = [user_data] }` prevents EC2 instance recreation. A `null_resource` with a config content hash trigger pushes updates via SSH automatically on `terraform apply`.
- **Secret changes** (passwords, RPC URLs): `aws_secretsmanager_secret_version` updates in-place. SSH into instance and re-run `pull-secrets.sh`, then restart services.
- **Instance type changes**: Triggers EC2 stop + start (expected).

## Workload ownership mode

This example defaults to:

- `compute_engine = "ec2"`
- `workload_mode = "terraform"`

Behavior by mode:

- `terraform`: EC2 instance + Docker Compose services are managed by Terraform.
- `external`: Terraform provisions infra and IAM handoff only; workload resources are not managed.

External mode still outputs `workload_handoff` v1 with SSH connection details for deploy pipelines.
