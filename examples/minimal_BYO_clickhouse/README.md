# Minimal ClickHouse BYODB Example

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│ AWS (us-east-1)                                                     │
│                                                                     │
│  ┌──────────────────────────── VPC (10.42.0.0/16) ───────────────┐  │
│  │                                                                │  │
│  │  ┌─ Private Subnets ────────────────────────────────────────┐  │  │
│  │  │                                                          │  │  │
│  │  │  ┌─ ECS Fargate Cluster (evm-cloud-clickhouse-dev) ──┐  │  │  │
│  │  │  │                                                    │  │  │  │
│  │  │  │  ┌─────────────────┐    ┌──────────────────────┐   │  │  │  │
│  │  │  │  │ eRPC Proxy      │    │ rindexer Indexer     │   │  │  │  │
│  │  │  │  │ (ECS Service)   │    │ (ECS Service)        │   │  │  │  │
│  │  │  │  │                 │    │                      │   │  │  │  │
│  │  │  │  │ Port 4000       │    │ 1 task (single-      │   │  │  │  │
│  │  │  │  │ 512 CPU/1024 MB │    │ writer constraint)   │   │  │  │  │
│  │  │  │  └────────┬────────┘    └──────────┬───────────┘   │  │  │  │
│  │  │  │           │                        │               │  │  │  │
│  │  │  └────────────────────────────────────────────────────┘  │  │  │
│  │  └──────────────────────────────────────────────────────────┘  │  │
│  │                                                                │  │
│  │  ┌─ S3 ─────────────────┐  ┌─ CloudWatch ────────────────┐   │  │
│  │  │ Config bucket         │  │ /ecs/.../erpc  (30d)        │   │  │
│  │  │  erpc/erpc.yaml       │  │ /ecs/.../indexer (30d)      │   │  │
│  │  │  rindexer/rindexer.yaml│ └─────────────────────────────┘   │  │
│  │  │  rindexer/abis/*.json │                                    │  │
│  │  └───────────────────────┘                                    │  │
│  │                                                                │  │
│  │  ┌─ Public Subnets ──────┐                                    │  │
│  │  │  Internet Gateway      │                                    │  │
│  │  └────────────────────────┘                                    │  │
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

### Config Storage

- S3 bucket (`evm-cloud-clickhouse-dev-config`) — versioned, all public access blocked
- S3 objects:
  - `erpc/erpc.yaml` — eRPC config (from `config/erpc.yaml`)
  - `rindexer/rindexer.yaml` — rindexer config (from `config/rindexer.yaml`)
  - `rindexer/abis/ERC20.json` — ABI (from `config/abis/ERC20.json`)

### Compute (shared ECS cluster)

- ECS Fargate cluster (`evm-cloud-clickhouse-dev`) with Container Insights

### eRPC proxy (rpc-proxy module)

- ECS Service (`evm-cloud-clickhouse-erpc`) — 1 task, Fargate
- 512 CPU / 1024 MiB memory
- Entrypoint: pulls `erpc.yaml` from S3, runs `erpc-server --config /tmp/erpc.yaml`
- Config: proxies Ethereum mainnet via `https://eth.llamarpc.com` with retry (5x) + hedge (2x)
- Listens on port 4000
- IAM task role with `s3:GetObject` on config bucket

### rindexer indexer (indexer module)

- ECS Service (`evm-cloud-clickhouse-indexer`) — 1 task, Fargate, desired_count = 1 (single-writer)
- 1024 CPU / 2048 MiB memory
- Entrypoint: pulls `rindexer.yaml` + `abis/` from S3, runs `rindexer start --path /tmp/project`
- Config: indexes USDT Transfer events on Ethereum mainnet (blocks 19M-19M+100)
- Storage backend: **ClickHouse (BYODB)** — connects to your external ClickHouse instance
- ClickHouse credentials injected as env vars (`CLICKHOUSE_URL`, `CLICKHOUSE_USER`, `CLICKHOUSE_PASSWORD`, `CLICKHOUSE_DB`)
- RPC_URL env var injected for `rindexer.yaml` `${RPC_URL}` interpolation
- IAM task role with `s3:GetObject` + `s3:ListBucket` on config bucket

### Logging

- 2 CloudWatch Log Groups: `/ecs/evm-cloud-clickhouse/dev/erpc` and `/ecs/evm-cloud-clickhouse/dev/indexer` (30-day retention)

## Differences from other examples

| | `minimal_rds` | `minimal_BYO_clickhouse` |
|---|---|---|
| **Database** | Managed RDS PostgreSQL (provisioned by Terraform) | External ClickHouse (Bring Your Own) |
| **DB credentials** | AWS Secrets Manager → ECS secrets injection | Env vars from tfvars (`CLICKHOUSE_*`) |
| **DB security group** | Yes (port 5432, indexer → RDS) | No (ClickHouse is external) |
| **Resources** | ~70 | ~62 |
| **Storage in rindexer.yaml** | `storage.postgres` | `storage.clickhouse` |

## Data Flow

```
eth.llamarpc.com
      │
  eRPC (port 4000)         ← erpc.yaml from S3
      │ (RPC_URL)
  rindexer                 ← rindexer.yaml + abis/ from S3
      │ (CLICKHOUSE_URL)
  ClickHouse (external)    ← creds from env vars
```

## Usage

```bash
# 1. Copy secrets template and fill in real values
cp secrets.auto.tfvars.example secrets.auto.tfvars

# 2. Initialize and plan
terraform init
terraform plan -var-file=minimal_clickhouse.tfvars

# 3. Apply
terraform apply -var-file=minimal_clickhouse.tfvars
```

Sensitive values (`indexer_rpc_url`, `indexer_clickhouse_password`) go in `secrets.auto.tfvars` which is gitignored and auto-loaded by Terraform.
