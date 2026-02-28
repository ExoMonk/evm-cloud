# Minimal Example

## Infrastructure

**Networking (VPC layer)**

- VPC (`10.42.0.0/16`) with DNS support
- 2 public subnets + 2 private subnets across `us-east-1a` and `us-east-1b`
- Internet Gateway for public subnet egress
- Route tables for public/private routing
- 4 Security Groups: erpc (port 4000), indexer (egress-only), database (port 5432 from indexer SG), bastion

**Database**

- RDS PostgreSQL 16.4 (db.t4g.micro) in private subnets
  - DB name: `rindexer`, user: `rindexer`
  - `manage_master_user_password = true` — AWS creates a Secrets Manager secret with credentials
  - Storage encrypted, 7-day backup retention, CloudWatch logs enabled
  - Enhanced monitoring (60s interval), Performance Insights enabled

**Config Storage**

- S3 bucket (`evm-cloud-example-dev-config`) — versioned, all public access blocked
- S3 objects:
  - `erpc/erpc.yaml` — eRPC config (`from config/erpc.yaml`)
  - `rindexer/rindexer.yaml` — rindexer config (`from config/rindexer.yaml`)
  - `rindexer/abis/ERC20.json` — ABI (from `config/abis/ERC20.json`)

**Compute (shared ECS cluster)**
- ECS Fargate cluster (`evm-cloud-example-dev`) with Container Insights

**eRPC proxy (rpc-proxy module)**
- ECS Service (`evm-cloud-example-erpc`) — 1 task, Fargate
- 512 CPU / 1024 MiB memory
- Entrypoint: pulls `erpc.yaml` from S3, runs `erpc-server --config /tmp/erpc.yaml`
- Config: proxies Ethereum mainnet via `https://eth.llamarpc.com` with retry (5x) + hedge (2x)
- Listens on port 4000
- IAM task role with `s3:GetObject` on config bucket

**rindexer indexer (indexer module)**
- ECS Service (`evm-cloud-example-indexer`) — 1 task, Fargate, desired_count = 1 (single-writer)
- 1024 CPU / 2048 MiB memory
- Entrypoint: pulls `rindexer.yaml` + `abis/` from S3, composes `DATABASE_URL` from Secrets Manager creds, runs `rindexer start --path /tmp/project`
- Config: indexes USDT Transfer events on Ethereum mainnet (blocks 19M–19M+100)
- RPC_URL env var injected for `rindexer.yaml` `${RPC_URL}` interpolation
- DB credentials injected from Secrets Manager via ECS secrets
- IAM task role with `s3:GetObject` + `s3:ListBucket` on config bucket

**Logging**
- 2 CloudWatch Log Groups: `/ecs/evm-cloud-example/dev/erpc` and `/ecs/evm-cloud-example/dev/indexer` (30-day retention)

## Data Flow at runtime

```
eth.llamarpc.com
      ↓
  eRPC (port 4000)         ← erpc.yaml from S3
      ↓ (RPC_URL)
  rindexer                 ← rindexer.yaml + abis/ from S3
      ↓ (DATABASE_URL)
  RDS PostgreSQL           ← creds from Secrets Manager
```
