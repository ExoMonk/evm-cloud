# k3s Deployer

Deploys evm-cloud workloads (eRPC + rindexer) to a k3s cluster using Helm CLI.

## Prerequisites

- `helm` >= 3.12
- `kubectl`
- `jq`
- `python3` (for config injection)
- Terraform has completed Phase 1 (`terraform apply` with `compute_engine = "k3s"`)
- A config directory with `erpc.yaml`, `rindexer.yaml`, and `abis/*.json`

## Usage

```bash
# Export handoff from Terraform
terraform output -json workload_handoff > handoff.json
chmod 0600 handoff.json

# Deploy workloads (--config-dir points to your erpc.yaml + rindexer.yaml + abis/)
./deployers/k3s/deploy.sh handoff.json --config-dir ./config

# Teardown (run before terraform destroy)
./deployers/k3s/teardown.sh handoff.json

# Then destroy infrastructure
terraform destroy
```

Alternatively, pipe directly to avoid writing credentials to disk:

```bash
terraform output -json workload_handoff | \
  ./deployers/k3s/deploy.sh /dev/stdin --config-dir ./config
```

## How It Works

The deploy script runs three steps:

1. **render-values.sh** — generates skeleton Helm values from handoff metadata (service names, ports, storage backend)
2. **populate-values-from-config-bundle.sh** (shared with EKS deployer) — injects real `erpc.yaml`, `rindexer.yaml`, and ABIs into the skeleton values
3. **helm upgrade --install** — deploys the populated charts to the k3s cluster

## Security

- **kubeconfig contains static cluster admin credentials** (~1 year validity). This is different from EKS which uses ephemeral STS tokens.
- `handoff.json` should be written with `0600` permissions or piped directly.
- **Use an encrypted Terraform state backend** (S3+KMS, Terraform Cloud, etc.). The state contains the kubeconfig.
- The `workload_handoff` output is marked `sensitive = true` in Terraform.

## Charts

Helm charts are shared with the EKS deployer at `deployers/charts/`:
- `rpc-proxy/` — eRPC deployment
- `indexer/` — rindexer deployment

## Lifecycle

```text
terraform apply          → Phase 1: Provisions host + installs k3s
deployers/k3s/deploy.sh  → Phase 2: Deploys workloads via Helm
deployers/k3s/teardown.sh → Uninstalls Helm releases
terraform destroy        → Removes host + k3s (also runs k3s-uninstall.sh via destroy provisioner)
```

The `terraform destroy` command includes a destroy-time provisioner that runs `k3s-uninstall.sh` on the host. However, running `teardown.sh` first is recommended for clean Helm release removal.
