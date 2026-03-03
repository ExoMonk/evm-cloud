# Multi-Node k3s + ClickHouse BYODB Example

## Architecture

```
┌───────────────────────────────────────────────────────────────────────────┐
│ AWS (us-east-1)                                                           │
│                                                                           │
│  ┌──────────────────────────── VPC (10.42.0.0/16) ────────────────────┐  │
│  │                                                                     │  │
│  │  ┌─ Public Subnets ─────────────────────────────────────────────┐  │  │
│  │  │                                                               │  │  │
│  │  │  ┌─ EC2: Server (t3.small)   ─────────────────────────────┐   │  │  │
│  │  │  │  k3s server (control plane + workloads)               │   │  │  │
│  │  │  │  ┌────────────────┐    ┌──────────────────────┐       │   │  │  │
│  │  │  │  │ eRPC Proxy     │    │ rindexer (live)      │       │   │  │  │
│  │  │  │  │ (K8s pod)      │    │ (K8s pod)            │       │   │  │  │
│  │  │  │  └────────────────┘    └──────────────────────┘       │   │  │  │
│  │  │  └───────────────────────────────────────────────────────┘   │  │  │
│  │  │                                                               │  │  │
│  │  │  ┌─ EC2: Worker (t3.small, spot).  ──────────────────────┐    │  │  │
│  │  │  │  k3s agent — label: evm-cloud/role=indexer           │    │  │  │
│  │  │  │  ┌──────────────────────┐                            │    │  │  │
│  │  │  │  │ rindexer (backfill)  │                            │    │  │  │
│  │  │  │  │ (K8s pod via affinity)│                           │    │  │  │
│  │  │  │  └──────────────────────┘                            │    │  │  │
│  │  │  └──────────────────────────────────────────────────────┘    │  │  │
│  │  │                                                               │  │  │
│  │  │  Flannel VXLAN (UDP 8472) connects all nodes                 │  │  │
│  │  │  Internet Gateway                                             │  │  │
│  │  └───────────────────────────────────────────────────────────────┘  │  │
│  │                                                                     │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────────────────┘

        External:
        ┌───────────────────┐         ┌──────────────────────┐
        │ eth.llamarpc.com  │         │ ClickHouse (BYODB)   │
        │ (upstream RPC)    │         │ clickhouse.example.com│
        └────────┬──────────┘         └──────────┬───────────┘
                 │                               │
                 ▼                               ▲
            eRPC proxy ──RPC_URL──▶ rindexer ────┘
```

## What's Different from `minimal_aws_k3s_byo_clickhouse`

| | `minimal_aws_k3s_byo_clickhouse` | **`aws_k3s_multi_byo_clickhouse`** |
| --- | --- | --- |
| **Nodes** | 1 EC2 (server only) | 2 EC2 (1 server + 1 spot worker) |
| **Node labels** | None | `evm-cloud/role=indexer` on worker |
| **Firewall** | SSH + k3s API + NodePort | + Flannel VXLAN (UDP 8472) + Kubelet (TCP 10250) |
| **Use case** | Dev / single-service | Live indexer on server + backfill on spot worker |

## Two-Phase Deployment

| Phase | Tool | What happens |
|-------|------|--------------|
| **Phase 1** | `terraform apply` | Provisions VPC, 2 EC2 instances, security groups. Installs k3s server, extracts node token, joins worker. Extracts kubeconfig. |
| **Phase 2** | `deployers/k3s/deploy.sh` | Reads the `workload_handoff` output, checks all nodes are Ready, deploys eRPC + rindexer via `helm upgrade --install`. |

## Usage

```bash
# 1) Move into this example
cd examples/aws_k3s_multi_byo_clickhouse

# 2) Copy secrets template and fill in real values
cp secrets.auto.tfvars.example secrets.auto.tfvars
# Edit secrets.auto.tfvars:
#   ssh_public_key             = "ssh-ed25519 AAAA..."
#   k3s_ssh_private_key_path   = "~/.ssh/id_ed25519"
#   indexer_clickhouse_password = "your-password"
#   indexer_clickhouse_url     = "https://your-clickhouse:8443"

# 3) Initialize and apply (Phase 1)
terraform init
terraform plan -var-file=k3s_multinode.tfvars
terraform apply -var-file=k3s_multinode.tfvars

# 4) Verify cluster nodes (requires your IP in k3s_api_allowed_cidrs)
KUBECONFIG=$(mktemp)
terraform output -json workload_handoff | jq -r '.runtime.k3s.kubeconfig_base64' | base64 -d > "$KUBECONFIG"
kubectl --kubeconfig="$KUBECONFIG" get nodes --show-labels
# Should show: server + backfill worker with evm-cloud/role=indexer label

# 5) Deploy workloads (Phase 2) — eRPC + live indexer only
terraform output -json workload_handoff | \
  ./../../deployers/k3s/deploy.sh /dev/stdin --config-dir ./config --instance indexer

# 5b) Later, deploy backfill on demand:
terraform output -json workload_handoff | \
  ./../../deployers/k3s/deploy.sh /dev/stdin --config-dir ./config --instance backfill --job

# 6) Verify pods
kubectl --kubeconfig="$KUBECONFIG" get pods -A
rm -f "$KUBECONFIG"

# 7) Teardown (reverse order)
terraform output -json workload_handoff | \
./../../deployers/k3s/teardown.sh /dev/stdin   # Remove Helm releases
terraform destroy -var-file=k3s_multinode.tfvars  # Drain worker, uninstall k3s, terminate EC2s
```

## Deploy Script Usage

The deploy script (`deployers/k3s/deploy.sh`) reads the Terraform `workload_handoff` output and deploys workloads via Helm. It requires `jq`, `helm`, `kubectl`, `base64`, and `python3` on your local machine.

```bash
# Deploy eRPC + live indexer (default workflow):
terraform output -json workload_handoff | \
  ./../../deployers/k3s/deploy.sh /dev/stdin --config-dir ./config --instance indexer

# Deploy backfill on demand (runs on worker spot node, exits on completion):
terraform output -json workload_handoff | \
  ./../../deployers/k3s/deploy.sh /dev/stdin --config-dir ./config --instance backfill --job

# Deploy everything at once (eRPC + live + backfill):
terraform output -json workload_handoff | \
  ./../../deployers/k3s/deploy.sh /dev/stdin --config-dir ./config
```

### Config directory layout

```
config/
├── erpc.yaml              # eRPC config
├── rindexer.yaml          # Live indexer config (default)
├── backfill/
│   └── rindexer.yaml      # Backfill indexer config (start_block → end_block)
└── abis/
    └── ERC20.json         # Shared ABIs (used by all instances)
```

The deployer matches config directories to instances via `config_key`. An instance with `config_key = "backfill"` uses `config/backfill/rindexer.yaml`. Instances without a `config_key` use `config/rindexer.yaml`.

## Worker Node Configuration

Workers are defined in `k3s_worker_nodes`:

```hcl
k3s_worker_nodes = [
  { name = "backfill", role = "indexer", instance_type = "t3.small", use_spot = true },
]
```

- **`use_spot = true`**: Launches a spot instance (~70% cheaper than on-demand). Recommended for interruptible workloads like backfill indexers. AWS gives a 2-minute interruption warning; the k3s agent drains the node gracefully.
- Available roles: `indexer`, `database`, `evm-node`, `monitoring`, `general`.
- Use Helm `affinity` values to schedule pods on specific node roles. For hard isolation, add taints alongside labels (not enforced by default).

## Security Notes

- **kubeconfig contains static cluster admin credentials** (~1 year validity). Use `terraform output -json` to access it.
- **node_token is stored in Terraform state only** (not in handoff JSON). Use an encrypted state backend.
- k3s API (port 6443) is restricted to VPC CIDR by default. Override with `k3s_api_allowed_cidrs`.
- Flannel VXLAN (UDP 8472) and Kubelet (TCP 10250) are restricted to VPC CIDR.

## Lifecycle

```
terraform apply              → Phase 1: VPC + 2 EC2s + k3s server + worker joins
deployers/k3s/deploy.sh     → Phase 2: Helm install eRPC + rindexer (live + backfill)
deployers/k3s/teardown.sh   → Uninstall Helm releases
terraform destroy            → Drain worker, delete node, uninstall k3s, terminate EC2s
```

On `terraform destroy`, the worker is drained and deleted from the cluster before its EC2 instance is terminated. The server runs `k3s-uninstall.sh` last.
