# EKS + ClickHouse BYODB Example

## Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│ AWS (us-east-1)                                                      │
│                                                                      │
│  ┌──────────────────────────── VPC (10.42.0.0/16) ────────────────┐  │
│  │                                                                │  │
│  │  ┌─ Private Subnets ────────────────────────────────────────┐  │  │
│  │  │                                                          │  │  │
│  │  │  ┌─ EKS Cluster (evm-cloud-eks-ch-dev) ───────────────┐  │  │  │
│  │  │  │                                                    │  │  │  │
│  │  │  │  ┌─────────────────┐    ┌──────────────────────┐   │  │  │  │
│  │  │  │  │ eRPC Proxy      │    │ rindexer Indexer     │   │  │  │  │
│  │  │  │  │ (Deployment)    │    │ (Deployment)         │   │  │  │  │
│  │  │  │  │                 │    │                      │   │  │  │  │
│  │  │  │  │ ConfigMap:      │    │ ConfigMap: config    │   │  │  │  │
│  │  │  │  │   erpc.yaml     │    │   rindexer.yaml      │   │  │  │  │
│  │  │  │  │                 │    │ ConfigMap: abis      │   │  │  │  │
│  │  │  │  │ Service:        │    │   ERC20.json         │   │  │  │  │
│  │  │  │  │   ClusterIP     │    │ Secret:              │   │  │  │  │
│  │  │  │  │   :4000         │    │   CLICKHOUSE_PASSWORD│   │  │  │  │
│  │  │  │  └────────┬────────┘    └──────────┬───────────┘   │  │  │  │
│  │  │  │           │                        │               │  │  │  │
│  │  │  │  Managed Node Group (t3.medium, 1-3 nodes)         │  │  │  │
│  │  │  └────────────────────────────────────────────────────┘  │  │  │
│  │  └──────────────────────────────────────────────────────────┘  │  │
│  │                                                                │  │
│  │  ┌─ Public Subnets ─────────────────────────────────────────┐  │  │
│  │  │  Internet Gateway                                        │  │  │
│  │  └──────────────────────────────────────────────────────────┘  │  │
│  └────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────┘

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

**Networking (VPC layer)**

- VPC (`10.42.0.0/16`) with DNS support
- 2 public subnets + 2 private subnets across `us-east-1a` and `us-east-1b`
- Internet Gateway for public subnet egress
- Route tables for public/private routing

**EKS Cluster**

- EKS cluster (`evm-cloud-eks-ch-dev`) with Kubernetes 1.29
- Single managed node group (`t3.medium`, 1-3 nodes, desired 1)
- OIDC provider enabled (IRSA-ready)
- Public API endpoint (Tier 0)

**eRPC proxy (K8s Deployment)**

- ConfigMap: `evm-cloud-eks-ch-erpc-config` — erpc.yaml content
- Deployment: `evm-cloud-eks-ch-erpc` — 1 replica
- Service: `evm-cloud-eks-ch-erpc` — ClusterIP on port 4000
- Config: proxies Ethereum mainnet via `https://eth.llamarpc.com` with retry (5x) + hedge (2x)

**rindexer indexer (K8s Deployment)**

- ConfigMap: `evm-cloud-eks-ch-indexer-config` — rindexer.yaml content
- ConfigMap: `evm-cloud-eks-ch-indexer-abis` — ABI files
- Secret: `evm-cloud-eks-ch-indexer-secrets` — `CLICKHOUSE_PASSWORD`
- Deployment: `evm-cloud-eks-ch-indexer` — 1 replica, Recreate strategy (single-writer)
- Config: indexes USDT Transfer events on Ethereum mainnet (blocks 19M-19M+100)
- Storage backend: **ClickHouse (BYODB)** — connects to your external ClickHouse instance
- Env vars: `RPC_URL`, `CLICKHOUSE_URL`, `CLICKHOUSE_USER`, `CLICKHOUSE_DB` (plain), `CLICKHOUSE_PASSWORD` (from Secret)

## Differences from other examples

| | `minimal_rds` | `minimal_BYO_clickhouse` | `eks_BYO_clickhouse` |
|---|---|---|---|
| **Compute** | ECS Fargate | ECS Fargate | EKS (Kubernetes) |
| **Database** | Managed RDS PostgreSQL | External ClickHouse | External ClickHouse |
| **Config delivery** | S3 pull at boot | S3 pull at boot | ConfigMap volume mount |
| **DB credentials** | Secrets Manager | Env vars | K8s Secret |
| **Logging** | CloudWatch Log Groups | CloudWatch Log Groups | `kubectl logs` |

## Data Flow

```
eth.llamarpc.com
      │
  eRPC (ClusterIP:4000)     ← erpc.yaml from ConfigMap
      │ (RPC_URL)
  rindexer                   ← rindexer.yaml + abis/ from ConfigMaps
      │ (CLICKHOUSE_URL)
  ClickHouse (external)      ← password from K8s Secret
```

## Usage

```bash
# 1. Copy secrets template and fill in real values
cp secrets.auto.tfvars.example secrets.auto.tfvars

# 2. Initialize and plan
terraform init
terraform plan -var-file=eks_clickhouse.tfvars

# 3. Apply
terraform apply -var-file=eks_clickhouse.tfvars

# 4. Configure kubectl
aws eks update-kubeconfig --name evm-cloud-eks-ch-dev --region us-east-1

# 5. Check pods
kubectl get pods
kubectl logs -l app=evm-cloud-eks-ch-indexer
```

Sensitive values (`indexer_rpc_url`, `indexer_clickhouse_password`) go in `secrets.auto.tfvars` which is gitignored and auto-loaded by Terraform.
