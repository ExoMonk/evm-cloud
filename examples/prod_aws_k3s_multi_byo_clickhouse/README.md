# Production Multi-Node k3s + ClickHouse BYODB + Secrets Manager

Production-grade example with AWS Secrets Manager + External Secrets Operator (ESO). ClickHouse passwords never appear in the handoff JSON or Helm values.

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
│  │  │  │  IAM instance profile → SM:GetSecretValue             │   │  │  │
│  │  │  │  ┌────────────────┐    ┌──────────────────────┐       │   │  │  │
│  │  │  │  │ eRPC Proxy     │    │ rindexer (live)      │       │   │  │  │
│  │  │  │  │ (K8s pod)      │    │ (K8s pod)            │       │   │  │  │
│  │  │  │  └────────────────┘    └──────────────────────┘       │   │  │  │
│  │  │  │  ┌────────────────┐    ┌──────────────────────┐       │   │  │  │
│  │  │  │  │ ESO            │    │ ClusterSecretStore   │       │   │  │  │
│  │  │  │  │ (K8s operator) │    │ → AWS SM (IMDS auth) │       │   │  │  │
│  │  │  │  └────────────────┘    └──────────────────────┘       │   │  │  │
│  │  │  └───────────────────────────────────────────────────────┘   │  │  │
│  │  │                                                               │  │  │
│  │  │  ┌─ EC2: Worker (t3.small, spot)  ──────────────────────┐    │  │  │
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
│                                                                           │
│  ┌─ AWS Secrets Manager ───────────────────────────────────────────────┐  │
│  │  evm-cloud/evm-cloud-k3s-prod/workload-env                         │  │
│  │  { CLICKHOUSE_URL, CLICKHOUSE_PASSWORD, CLICKHOUSE_USER, ... }     │  │
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

| | `minimal_aws_k3s_byo_clickhouse` | **`prod_aws_k3s_multi_byo_clickhouse`** |
| --- | --- | --- |
| **Nodes** | 1 EC2 (server only) | 2 EC2 (1 server + 1 spot worker) |
| **Secrets** | `inline` (password in handoff) | `provider` (AWS SM + ESO, no password in handoff) |
| **IAM** | None | Instance profile with SM:GetSecretValue |
| **ESO** | Not installed | Installed, manages ExternalSecret CRs |
| **Node labels** | None | `evm-cloud/role=indexer` on worker |
| **Firewall** | SSH + k3s API + NodePort | + Flannel VXLAN (UDP 8472) + Kubelet (TCP 10250) |
| **Use case** | Dev / single-service | Production: live + backfill, hardened secrets |

## Secrets Flow

```
terraform apply
  → Creates SM secret (evm-cloud/<project>/workload-env)
  → Creates IAM instance profile (SM:GetSecretValue)
  → Installs ESO Helm chart
  → Outputs workload_handoff (NO passwords)

deploy.sh
  → Waits for ESO CRDs + deployment ready
  → Creates ClusterSecretStore (AWS SM via IMDS auth)
  → Helm install → ExternalSecret CR
  → ESO syncs SM → K8s Secret
  → Pods mount secret
```

## Two-Phase Deployment

| Phase | Tool | What happens |
|-------|------|--------------|
| **Phase 1** | `terraform apply` | Provisions VPC, 2 EC2 instances, security groups, SM secret, IAM. Installs k3s + ESO. |
| **Phase 2** | `deployers/k3s/deploy.sh` | Creates ClusterSecretStore, deploys eRPC + rindexer with ExternalSecret (ESO syncs from SM). |

## Usage

```bash
# 1) Move into this example
cd examples/prod_aws_k3s_multi_byo_clickhouse

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

export KUBECONFIG=$(terraform output -json workload_handoff | jq -r '.runtime.k3s.kubeconfig_base64' | base64 -d > /tmp/k3s-kubeconfig && echo /tmp/k3s-kubeconfig)

# 4) Verify cluster nodes
kubectl get nodes --show-labels
# Should show: server + backfill worker with evm-cloud/role=indexer label

# 5) Deploy workloads (Phase 2) — ESO + Monitoring + eRPC + live indexer only
terraform output -json workload_handoff | \
  ./../../deployers/k3s/deploy.sh /dev/stdin --config-dir ./config --instance indexer
# deploy.sh will:
#   - Wait for ESO CRDs and deployment
#   - Create ClusterSecretStore → AWS SM
#   - Helm install with secretsMode=provider (ExternalSecret, not Secret)

# 5b) Later, deploy backfill on demand:
terraform output -json workload_handoff | \
  ./../../deployers/k3s/deploy.sh /dev/stdin --config-dir ./config --instance backfill --job

# 6) Verify pods + ExternalSecret sync
kubectl get pods -A
kubectl get externalsecrets -A
# STATUS should show "SecretSynced"
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

- **Secrets Manager**: ClickHouse credentials stored in SM, synced via ESO. No passwords in handoff or Helm values.
- **IAM instance profile**: Scoped to `secretsmanager:GetSecretValue` on `evm-cloud/<project>/*` secrets only.
- **kubeconfig contains static cluster admin credentials** (~1 year validity). Use `terraform output -json` to access it.
- **node_token is stored in Terraform state only** (not in handoff JSON). Use an encrypted state backend.
- k3s API (port 6443) is restricted to VPC CIDR by default. Override with `k3s_api_allowed_cidrs`.
- Flannel VXLAN (UDP 8472) and Kubelet (TCP 10250) are restricted to VPC CIDR.
- `handoff.json` is automatically `chmod 0600` by `deploy.sh`.

## Lifecycle

```
terraform apply              → Phase 1: VPC + 2 EC2s + k3s + ESO + SM secret + IAM
deployers/k3s/deploy.sh     → Phase 2: ClusterSecretStore + Helm install (ExternalSecret)
deployers/k3s/teardown.sh   → Uninstall Helm releases
terraform destroy            → Drain worker, delete node, uninstall k3s, terminate EC2s
```

On `terraform destroy`, the worker is drained and deleted from the cluster before its EC2 instance is terminated. The server runs `k3s-uninstall.sh` last.

## Monitoring

This example enables monitoring by default (`monitoring_enabled = true`). Phase 2 (`deploy.sh`) installs kube-prometheus-stack with Prometheus, Grafana, Alertmanager, and 3 auto-provisioned dashboards (rindexer, eRPC, infrastructure).

### Accessing Grafana

Without ingress (`grafana_hostname` not set), use port-forward:

```bash
kubectl port-forward -n monitoring svc/monitoring-grafana 3000:80
# Open http://localhost:3000
# Default credentials: admin / prom-operator
```

With Cloudflare ingress:

```hcl
# In secrets.auto.tfvars or k3s_multinode.tfvars:
grafana_hostname = "grafana.yourdomain.com"
```

Then add a DNS A record in Cloudflare pointing `grafana.yourdomain.com` to the EC2 public IP (proxied). Get the IP from:

```bash
terraform output -json workload_handoff | jq -r '.runtime.k3s.cluster_endpoint'
```

### Verifying monitoring pods

```bash
kubectl get pods -n monitoring
```

Expected pods:

- `prometheus-monitoring-kube-prometheus-prometheus-0`
- `monitoring-grafana-*`
- `monitoring-kube-prometheus-operator-*`
- `monitoring-kube-state-metrics-*`
- `monitoring-prometheus-node-exporter-*` (one per node)
- `alertmanager-monitoring-kube-prometheus-alertmanager-0`

### Optional: Alert routing

```hcl
alertmanager_route_target              = "slack"
alertmanager_slack_webhook_secret_name = "slack-webhook"
alertmanager_slack_channel             = "#evm-alerts"
```

Create the Slack webhook secret before deploying:

```bash
kubectl create secret generic slack-webhook \
  --namespace monitoring \
  --from-literal=webhook_url="https://hooks.slack.com/services/T.../B.../xxx"
```

### Optional: Log aggregation

```hcl
loki_enabled = true
```

See the [Observability Guide](../../documentation/docs/pages/docs/guides/observability.mdx) for full details on dashboards, alert rules, and configuration.
