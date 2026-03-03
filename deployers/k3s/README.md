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

## Secrets Mode

The deployer supports three secrets modes, controlled by the `secrets_mode` Terraform variable:

| Mode | Handoff Contains | What deploy.sh Does |
|------|-----------------|---------------------|
| `inline` (default) | Database passwords | Injects passwords into Helm values → K8s Secret |
| `provider` | SM ARN + region (no passwords) | Waits for ESO, creates ClusterSecretStore → ExternalSecret (references SM secret by ARN) |
| `external` | Store name + secret key (no passwords) | Waits for ESO, verifies your ClusterSecretStore exists → ExternalSecret (references your secret key) |

When `secrets_mode != "inline"`, the deploy script:
1. Waits for ESO CRDs to be registered (120s timeout)
2. Waits for the ESO deployment to be ready
3. Creates or verifies the ClusterSecretStore
4. Deploys charts with `secretsMode` set — the chart creates an `ExternalSecret` instead of a `Secret`

See the [Secrets Management guide](https://evm-cloud.xyz/docs/guides/secrets-management) for full details.

## Ingress & TLS

The deployer supports automatic TLS termination via the `ingress_mode` Terraform variable:

| Mode | What deploy.sh Does |
|------|---------------------|
| `none` (default) | No ingress setup |
| `cloudflare` | Installs ingress-nginx, creates Cloudflare origin TLS secret, configures CF-Connecting-IP forwarding |
| `ingress_nginx` | Installs ingress-nginx + cert-manager, creates Let's Encrypt ClusterIssuer (staging or prod) |

The `caddy` mode is handled by the EC2/bare_metal Docker Compose path and is not applicable to k3s.

When ingress is enabled, the Helm charts create Kubernetes `Ingress` resources that route traffic to the appropriate service.

## Security

- **kubeconfig contains static cluster admin credentials** (~1 year validity). This is different from EKS which uses ephemeral STS tokens.
- `handoff.json` is automatically `chmod 0600` by `deploy.sh`. When writing manually, set permissions accordingly.
- **Use an encrypted Terraform state backend** (S3+KMS, Terraform Cloud, etc.). The state contains the kubeconfig.
- The `workload_handoff` output is marked `sensitive = true` in Terraform.
- With `secrets_mode = "provider"` or `"external"`, database passwords are removed from the handoff entirely.

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
