# Minimal RDS Example

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│ AWS (us-east-1)                                                     │
│                                                                     │
│  ┌──────────────────────────── VPC (10.42.0.0/16) ───────────────┐  │
│  │                                                                │  │
│  │  ┌─ Public Subnets ───────────────────────────────────────┐   │  │
│  │  │                                                         │   │  │
│  │  │  ┌─ EC2 Instance (t3.medium) ────────────────────────┐  │   │  │
│  │  │  │                                                    │  │   │  │
│  │  │  │  Docker Compose (bridge network: evm-cloud)        │  │   │  │
│  │  │  │                                                    │  │   │  │
│  │  │  │  ┌─────────────────┐    ┌──────────────────────┐   │  │   │  │
│  │  │  │  │ eRPC Proxy      │    │ rindexer Indexer     │   │  │   │  │
│  │  │  │  │ (container)     │    │ (container)          │   │  │   │  │
│  │  │  │  │                 │    │                      │   │  │   │  │
│  │  │  │  │ Port 4000       │    │ depends_on: erpc     │   │  │   │  │
│  │  │  │  │ mem_limit: 1g   │    │ mem_limit: 2g        │   │  │   │  │
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
│  │  ┌─ Private Subnets ──────────────────────────────────────┐   │  │
│  │  │                                                         │   │  │
│  │  │  ┌───────────────────┐                                  │   │  │
│  │  │  │ RDS PostgreSQL    │  DATABASE_URL composed from      │   │  │
│  │  │  │ 16.4 (t4g.micro)  │  Secrets Manager master creds   │   │  │
│  │  │  │ db: rindexer      │  → stored in SM → pulled to .env│   │  │
│  │  │  └───────────────────┘                                  │   │  │
│  │  └─────────────────────────────────────────────────────────┘   │  │
│  │                                                                │  │
│  │  ┌─ IAM ──────────────────┐  ┌─ CloudWatch ────────────────┐  │  │
│  │  │ EC2 instance role       │  │ /evm-cloud/...  (30d)       │  │  │
│  │  │  logs:PutLogEvents      │  └─────────────────────────────┘  │  │
│  │  │  sm:GetSecretValue      │                                   │  │
│  │  └─────────────────────────┘  ┌─ Secrets Manager ───────────┐  │  │
│  │                                │ DATABASE_URL (from RDS)     │  │  │
│  │                                │ RPC_URL                     │  │  │
│  │                                └─────────────────────────────┘  │  │
│  └────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘

        External:
        ┌───────────────────┐
        │ eth.llamarpc.com  │
        │ (upstream RPC)    │
        └────────┬──────────┘
                 │
                 ▼
            eRPC proxy ──RPC_URL──▶ rindexer ──DATABASE_URL──▶ RDS
```

## What Gets Deployed

### Networking (VPC layer)

- VPC (`10.42.0.0/16`) with DNS support
- 2 public subnets + 2 private subnets across `us-east-1a` and `us-east-1b`
- Internet Gateway for public subnet egress
- Route tables for public/private routing
- 4 Security Groups: erpc (port 4000), indexer (egress-only), database (port 5432 from indexer SG), bastion

### Database

- RDS PostgreSQL 16.4 (db.t4g.micro) in private subnets
  - DB name: `rindexer`, user: `rindexer`
  - `manage_master_user_password = true` — AWS creates a Secrets Manager secret with credentials
  - Storage encrypted, CloudWatch logs enabled
  - Enhanced monitoring (60s interval), Performance Insights enabled

### IAM

- EC2 instance role with policies:
  - `logs:CreateLogStream` + `logs:PutLogEvents` on CloudWatch log group
  - `secretsmanager:GetSecretValue` on `evm-cloud/<project>/*`
- IAM instance profile attached to the EC2 instance

### Compute (EC2 + Docker Compose)

- Single EC2 instance (`t3.medium` default, configurable via `ec2_instance_type`)
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
- Memory limit: 1g (configurable via `ec2_rpc_proxy_mem_limit`)

### rindexer indexer (Docker container)

- Container: `rindexer` — `ghcr.io/joshstevens19/rindexer:latest`
- Config: bind-mounted `/opt/evm-cloud/config/` (rindexer.yaml + abis/)
- Indexes USDT Transfer events on Ethereum mainnet (blocks 19M-19M+100)
- Storage backend: **Managed RDS PostgreSQL** — `DATABASE_URL` composed from RDS Secrets Manager credentials
- `depends_on: erpc` with `service_healthy` condition
- Memory limit: 2g (configurable via `ec2_indexer_mem_limit`)

### Logging

- CloudWatch Log Group: `/evm-cloud/<project>-<env>` (30-day retention)
- Docker `awslogs` driver ships container logs (streams: `erpc`, `rindexer`)

## Data Flow

```
eth.llamarpc.com
      │
  eRPC (port 4000)         ← erpc.yaml bind-mounted from /opt/evm-cloud/config/
      │ (RPC_URL=http://erpc:4000, Docker Compose network)
  rindexer                  ← rindexer.yaml + abis/ bind-mounted
      │ (DATABASE_URL)
  RDS PostgreSQL            ← creds from Secrets Manager → .env
```

## Usage

```bash
# 1) Move into this example
cd examples/minimal_rds

# 2) Copy secrets template and fill in real values
cp secrets.auto.tfvars.example secrets.auto.tfvars
# Edit secrets.auto.tfvars:
# - ssh_public_key = "ssh-ed25519 AAAA..."

# 3) Initialize Terraform
terraform init

# 4) Review plan
terraform plan -var-file=minimal.tfvars

# 5) Apply
terraform apply -var-file=minimal.tfvars

# 6) SSH into the instance
ssh -i ~/.ssh/your-key ec2-user@<public-ip>

# 7) Check running containers
sudo docker compose -f /opt/evm-cloud/docker-compose.yml ps

# 8) View logs
sudo docker compose -f /opt/evm-cloud/docker-compose.yml logs -f

# 9) (Optional) Destroy when done
terraform destroy -var-file=minimal.tfvars
```

## Verifying indexer data (querying RDS)

RDS is in a private subnet — query it via SSH tunnel through the EC2 instance.

```bash
# 1) Get connection details from Terraform outputs + Secrets Manager
RDS_ENDPOINT=$(terraform output -json postgres | jq -r '.endpoint')
RDS_SECRET_ARN=$(terraform output -json postgres | jq -r '.master_secret_arn')
RDS_PASSWORD=$(aws secretsmanager get-secret-value \
  --secret-id "$RDS_SECRET_ARN" \
  --query 'SecretString' --output text | jq -r '.password')
EC2_IP=$(terraform output -json workload_handoff | jq -r '.runtime.ec2.public_ip')

# 2) Open SSH tunnel (local port 5432 → RDS via EC2)
ssh -L 5432:${RDS_ENDPOINT}:5432 -i ~/.ssh/your-key ec2-user@${EC2_IP} -N &

# 3) List tables created by rindexer
psql "postgresql://rindexer:${RDS_PASSWORD}@localhost:5432/rindexer" \
  -c "SELECT table_name FROM information_schema.tables WHERE table_schema='public';"

# 4) Check indexed data
psql "postgresql://rindexer:${RDS_PASSWORD}@localhost:5432/rindexer" \
  -c "SELECT COUNT(*) FROM transfer_events;"
```

Alternatively, query from the EC2 instance directly:

```bash
ssh -i ~/.ssh/your-key ec2-user@<public-ip>
sudo yum install -y postgresql16
psql "postgresql://rindexer:<password>@<rds-endpoint>:5432/rindexer"
```

## Lifecycle behavior

- **Config changes** (erpc.yaml, rindexer.yaml, ABIs, mem limits): `lifecycle { ignore_changes = [user_data] }` prevents EC2 instance recreation. Update via SSH.
- **Secret changes** (passwords, RPC URLs): `aws_secretsmanager_secret_version` updates in-place. SSH into instance and re-run `pull-secrets.sh`, then restart services.
- **Instance type changes**: Triggers EC2 stop + start (expected).
- **Destroy**: Clean teardown in dev — `ec2_secret_recovery_window_in_days = 0` (immediate secret deletion), `deletion_protection = false`, `skip_final_snapshot = true`, `backup_retention = 0`.

## Workload ownership mode

This example defaults to:

- `compute_engine = "ec2"`
- `workload_mode = "terraform"`

Behavior by mode:

- `terraform`: EC2 instance + Docker Compose services are managed by Terraform.
- `external`: Terraform provisions infra and IAM handoff only; workload resources are not managed.

In both modes, `workload_handoff` is emitted. In external mode, use `workload_handoff.identity` and `workload_handoff.compute` SSH details to wire your external deployer.
