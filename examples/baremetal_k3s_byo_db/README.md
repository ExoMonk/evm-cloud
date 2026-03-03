# Bare Metal k3s + PostgreSQL BYODB

Single-node k3s on an existing VPS with an external PostgreSQL database. Simplest bare-metal Kubernetes setup — ideal for small projects on Hetzner, Latitude.sh, OVH, DigitalOcean, etc.

## Architecture

```
┌────────────────────────────────────────────────────────────────┐
│ Your VPS (bare metal / cloud VM)                               │
│                                                                │
│  ┌─ k3s single-node cluster ────────────────────────────────┐  │
│  │                                                           │  │
│  │  ┌────────────────┐    ┌──────────────────────┐          │  │
│  │  │ eRPC Proxy     │    │ rindexer (indexer)    │          │  │
│  │  │ (K8s pod)      │    │ (K8s pod)             │          │  │
│  │  └────────────────┘    └──────────────────────┘          │  │
│  │                                                           │  │
│  └───────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────┘

         External:
         ┌───────────────────┐         ┌──────────────────────┐
         │ eth.llamarpc.com  │         │ PostgreSQL (BYODB)   │
         │ (upstream RPC)    │         │ your-db-host:5432    │
         └────────┬──────────┘         └──────────┬───────────┘
                  │                               │
                  ▼                               ▲
             eRPC proxy ──RPC_URL──▶ rindexer ────┘
```

## Two-Phase Deployment

| Phase | Tool | What happens |
|-------|------|--------------|
| **Phase 1** | `terraform apply` | Installs k3s on your VPS via SSH. |
| **Phase 2** | `deployers/k3s/deploy.sh` | Deploys eRPC + rindexer via Helm. |

## Network Prerequisites

Unlike AWS examples, **you manage firewall rules** on your VPS:

| Port | Protocol | Purpose |
|------|----------|---------|
| 22 | TCP | SSH (Terraform provisioner) |
| 6443 | TCP | k3s API (kubectl, deploy.sh) |

**Database access is your responsibility.** Ensure your PostgreSQL instance is reachable from the VPS — evm-cloud does not provision or configure the database in bare metal mode.

```bash
# Example: ufw setup on your VPS
sudo ufw allow 22/tcp
sudo ufw allow 6443/tcp
sudo ufw enable
```

## Usage

```bash
# 1) Move into this example
cd examples/baremetal_k3s_byo_db

# 2) Copy secrets template and fill in real values
cp secrets.auto.tfvars.example secrets.auto.tfvars
# Edit secrets.auto.tfvars:
#   bare_metal_host                 = "203.0.113.42"
#   bare_metal_ssh_private_key_path = "~/.ssh/id_ed25519"
#   indexer_postgres_url            = "postgres://user:pass@host:5432/db"

# 3) Initialize and apply (Phase 1)
terraform init
terraform apply -var-file=bare_metal_k3s.tfvars

# 4) Deploy workloads (Phase 2)
terraform output -json workload_handoff | \
  ./../../deployers/k3s/deploy.sh /dev/stdin --config-dir ./config

# 5) Verify
export KUBECONFIG=$(terraform output -json workload_handoff | jq -r '.runtime.k3s.kubeconfig_base64' | base64 -d > /tmp/k3s-kubeconfig && echo /tmp/k3s-kubeconfig)
kubectl get pods -A

# 6) Teardown
terraform output -json workload_handoff | \
  ./../../deployers/k3s/teardown.sh /dev/stdin
terraform destroy -var-file=bare_metal_k3s.tfvars
```

## Upgrading to External Secrets

For production deployments with a secret backend (Vault, 1Password, Doppler), switch to `secrets_mode = "external"`:

```hcl
secrets_mode               = "external"
external_secret_store_name = "my-vault-store"
external_secret_key        = "prod/evm-cloud/workload-env"
```

See the [Secrets Management guide](https://evm-cloud.xyz/docs/guides/secrets-management) for setup details.

## Security Notes

- **No AWS dependency**: Runs entirely on bare metal. No IAM, no VPC, no cloud API calls at runtime.
- **Inline secrets**: DATABASE_URL flows through Terraform state → handoff → K8s Secret. Use an encrypted remote backend for production.
- **kubeconfig contains static cluster admin credentials** (~1 year validity). Use `terraform output -json` to access it.
- **k3s API is exposed on port 6443** on your VPS. Restrict access via your VPS firewall.
- Ensure your PostgreSQL endpoint is reachable from the VPS.
