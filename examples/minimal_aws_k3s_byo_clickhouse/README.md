# Minimal k3s + ClickHouse BYODB Example

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│ AWS (us-east-1)                                                     │
│                                                                     │
│  ┌──────────────────────────── VPC (10.42.0.0/16) ───────────────┐  │
│  │                                                                │  │
│  │  ┌─ Public Subnets ───────────────────────────────────────┐   │  │
│  │  │                                                         │   │  │
│  │  │  ┌─ EC2 Instance (t3.medium) ─────────────────────────┐  │   │  │
│  │  │  │                                                    │  │   │  │
│  │  │  │  k3s (single-node Kubernetes cluster)              │  │   │  │
│  │  │  │                                                    │  │   │  │
│  │  │  │  ┌─────────────────┐    ┌──────────────────────┐   │  │   │  │
│  │  │  │  │ eRPC Proxy      │    │ rindexer Indexer     │   │  │   │  │
│  │  │  │  │ (K8s pod)       │    │ (K8s pod)            │   │  │   │  │
│  │  │  │  │                 │    │                      │   │  │   │  │
│  │  │  │  │ Service: 4000   │    │ depends_on: erpc     │   │  │   │  │
│  │  │  │  └────────┬────────┘    └──────────┬───────────┘   │  │   │  │
│  │  │  │           │ ClusterIP svc          │               │  │   │  │
│  │  │  │           └────────────────────────┘               │  │   │  │
│  │  │  │                                                    │  │   │  │
│  │  │  │  k3s API: port 6443 (restricted to VPC CIDR)       │  │   │  │
│  │  │  │  NodePort range: 30000-32767                       │  │   │  │
│  │  │  └────────────────────────────────────────────────────┘  │   │  │
│  │  │                                                         │   │  │
│  │  │  Internet Gateway                                       │   │  │
│  │  └─────────────────────────────────────────────────────────┘   │  │
│  │                                                                │  │
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

## Two-Phase Deployment

Unlike the EC2 + Docker Compose examples, k3s uses a two-phase workflow:

| Phase | Tool | What happens |
|-------|------|--------------|
| **Phase 1** | `terraform apply` | Provisions VPC, EC2, security groups. Installs k3s on the instance via SSH. Extracts kubeconfig. |
| **Phase 2** | `deployers/k3s/deploy.sh` | Reads the `workload_handoff` output, deploys eRPC + rindexer via `helm upgrade --install`. |

This separation avoids the Terraform kubernetes provider chicken-and-egg problem — k3s kubeconfig doesn't exist until after the host is provisioned.

## What Gets Deployed

### Phase 1 (Terraform)

**Networking:**
- VPC (`10.42.0.0/16`) with DNS support
- 2 public subnets + 2 private subnets across `us-east-1a` and `us-east-1b`
- Internet Gateway for public subnet egress

**Security Groups (k3s-specific):**
- SSH (port 22) — restricted to VPC CIDR
- k3s API (port 6443) — restricted to VPC CIDR (configurable via `k3s_api_allowed_cidrs`)
- NodePort services (30000-32767) — restricted to VPC CIDR
- Egress: all outbound (k3s needs to pull container images)

**Compute:**
- Single EC2 instance (`t3.medium` default)
- Ubuntu 22.04 LTS AMI (k3s prefers Debian-based)
- SSH key pair from `ssh_public_key` variable
- 30GB gp3 encrypted root volume
- IMDSv2 enforced

**k3s Installation:**
- Binary downloaded with checksum verification (sha256)
- Hardened config: `--secrets-encryption`, `--write-kubeconfig-mode=0600`
- Disabled: traefik, servicelb (use your own ingress)
- TLS SAN includes the instance public IP
- Readiness check waits up to 120s for node Ready status
- Kubeconfig extracted and included in `workload_handoff` output

### Phase 2 (Helm Deployer)

**eRPC proxy (Helm release):**
- Chart: `deployers/charts/rpc-proxy`
- Proxies Ethereum mainnet via public RPC endpoints
- Configurable via `config/erpc.yaml`

**rindexer indexer (Helm release):**
- Chart: `deployers/charts/indexer`
- Indexes USDT Transfer events on Ethereum mainnet
- Storage backend: ClickHouse (BYODB)
- Configurable via `config/rindexer.yaml` + `config/abis/`

## Differences from Other Examples

| | `minimal_byo_clickhouse` | **`minimal_aws_k3s_byo_clickhouse`** | `eks_byo_clickhouse` |
|---|---|---|---|
| **Compute** | EC2 + Docker Compose | EC2 + k3s (Kubernetes) | EKS (managed K8s) |
| **K8s control plane cost** | N/A | $0 (k3s) | ~$75/mo (EKS) |
| **Database** | External ClickHouse | External ClickHouse | External ClickHouse |
| **Workload deployment** | Terraform (cloud-init) | Helm CLI (Phase 2) | Helm CLI or Terraform |
| **Config delivery** | cloud-init + bind mounts | Helm values → ConfigMap | Helm values → ConfigMap |
| **Credentials** | Secrets Manager → `.env` | Helm values → K8s Secret | K8s Secret |

## Usage

```bash
# 1) Move into this example
cd examples/minimal_aws_k3s_byo_clickhouse

# 2) Copy secrets template and fill in real values
cp secrets.auto.tfvars.example secrets.auto.tfvars
# Edit secrets.auto.tfvars:
#   ssh_public_key             = "ssh-ed25519 AAAA..."
#   k3s_ssh_private_key_path   = "~/.ssh/id_ed25519"
#   indexer_clickhouse_password = "your-password"
#   indexer_clickhouse_url     = "https://your-clickhouse:8443"

# 3) Initialize and apply (Phase 1)
terraform init
terraform plan -var-file=minimal_k3.tfvars
terraform apply -var-file=minimal_k3.tfvars

# 4) Deploy workloads (Phase 2)
terraform output -json workload_handoff | \
  ./../../deployers/k3s/deploy.sh /dev/stdin --config-dir ./config

# Or write to file (set restrictive perms — contains kubeconfig):
terraform output -json workload_handoff > handoff.json
chmod 0600 handoff.json
./../../deployers/k3s/deploy.sh handoff.json --config-dir ./config

# 5) Verify pods
KUBECONFIG=$(mktemp)
terraform output -json workload_handoff | jq -r '.runtime.k3s.kubeconfig_base64' | base64 -d > "$KUBECONFIG"
kubectl --kubeconfig="$KUBECONFIG" get pods
rm -f "$KUBECONFIG"

# 6) Teardown (reverse order)
./../../deployers/k3s/teardown.sh handoff.json   # Remove Helm releases
terraform destroy -var-file=minimal_k3.tfvars     # Remove infra + k3s
rm -f handoff.json
```

## Security Notes

- **kubeconfig contains static cluster admin credentials** (~1 year validity). This is different from EKS which uses ephemeral STS tokens.
- The `workload_handoff` output is marked `sensitive = true`. Use `terraform output -json` to access it.
- `handoff.json` should be written with `0600` permissions or piped directly to avoid writing credentials to disk.
- **Use an encrypted Terraform state backend** (S3+KMS, Terraform Cloud, etc.) — the state contains the kubeconfig.
- k3s API (port 6443) is restricted to VPC CIDR by default. Override with `k3s_api_allowed_cidrs` if you need external access.

## Config Updates (post-deploy)

Since workloads are deployed via Helm, update configs by re-running the deployer:

```bash
# Edit config files
vim config/erpc.yaml
vim config/rindexer.yaml

# Re-deploy (helm upgrade is idempotent)
terraform output -json workload_handoff | ./../../deployers/k3s/deploy.sh /dev/stdin
```

## Lifecycle

```
terraform apply              → Phase 1: VPC + EC2 + k3s install + kubeconfig
deployers/k3s/deploy.sh     → Phase 2: Helm install eRPC + rindexer
deployers/k3s/teardown.sh   → Uninstall Helm releases
terraform destroy            → Runs k3s-uninstall.sh on host, then terminates EC2
```

The `terraform destroy` command includes a destroy-time provisioner that runs `k3s-uninstall.sh` on the host. Running `teardown.sh` first is recommended for clean Helm release removal.
