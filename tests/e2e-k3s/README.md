# E2E k3s Tests

End-to-end tests that deploy workloads on a **persistent k3s VPS** via the real deployer pipeline (`deployers/k3s/deploy.sh`) and validate pods, networking, and k3s-specific behavior.

## Test Tiers

| Target | Cluster | Per-run cost | Infra cost |
|--------|---------|-------------|------------|
| `make qa` | none | $0 | $0 |
| `make test-k8s` | kind (local) | $0 | $0 |
| **`make test-e2e-k8s`** | **k3s (AWS EC2)** | **$0** | **~$17/mo** |

## Prerequisites

- `kubectl`, `helm`, `jq`, `curl` installed
- A persistent k3s VPS provisioned (see [Infrastructure Setup](#infrastructure-setup))
- A kubeconfig file for the k3s cluster

## Quick Start

```bash
# Set kubeconfig and run
E2E_KUBECONFIG=~/.kube/evm-cloud-e2e make test-e2e-k8s
```

## Infrastructure Setup

The E2E VPS is provisioned **once** and kept alive ($0/run cost, <5min test time).

### 1. Provision the VPS

```bash
cd tests/e2e-k3s/infra
cp secrets.auto.tfvars.example secrets.auto.tfvars
# Edit secrets.auto.tfvars:
#   - ssh_public_key: your SSH public key
#   - k3s_ssh_private_key_path: path to matching private key
#   - k3s_api_allowed_cidrs: your IP (curl -s ifconfig.me)

terraform init
terraform apply -var-file=e2e.tfvars
```

### 2. Extract the kubeconfig

```bash
terraform output -json workload_handoff \
  | jq -r '.runtime.k3s.kubeconfig_base64' \
  | base64 -d > ~/.kube/evm-cloud-e2e
chmod 0600 ~/.kube/evm-cloud-e2e

# Verify
KUBECONFIG=~/.kube/evm-cloud-e2e kubectl get nodes
```

### 3. Run tests

```bash
E2E_KUBECONFIG=~/.kube/evm-cloud-e2e make test-e2e-k8s
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `E2E_KUBECONFIG` | — | **Required.** Path to k3s kubeconfig file |
| `E2E_TIMEOUT` | `300` | Overall test timeout in seconds |
| `GITHUB_RUN_ID` | `$(date +%s)` | Used for unique namespace naming |

## Test Phases

| Phase | What it tests |
|-------|---------------|
| 0 | Prerequisites — tools, kubeconfig, deployer scripts |
| 1 | Cluster health — node Ready, CoreDNS, system pods, k3s version |
| 2 | Deploy — synthetic handoff JSON → `deploy.sh` → Helm releases |
| 3 | Resource assertions — ConfigMaps, Deployments, Services, Secrets, env vars, volumes |
| 4 | Runtime — eRPC Running + HTTP via port-forward, rindexer CrashLoop (expected) |
| 4.5 | Upgrade — config change → helm upgrade → verify new config active |
| 5 | Networking — NodePort in-cluster, DNS resolution via Job |
| 6 | k3s-specific — local-path PVC with pod mount, pod restart resilience |
| 7 | Teardown — `teardown.sh`, verify clean, delete namespace |

## ClickHouse Strategy

rindexer **CrashLoops on purpose** — we don't provision a ClickHouse instance. The tests validate the deployer creates correct K8s resources (ConfigMaps, Deployments, Secrets, Services) and that the container image pulled and ran. This keeps infra cost at ~$17/mo instead of ~$45/mo.

## CI Setup (GitHub Actions)

### Required secrets

| Secret | Value | How to obtain |
|--------|-------|---------------|
| `E2E_KUBECONFIG_B64` | Base64-encoded kubeconfig (scoped SA) | `terraform output -raw e2e_kubeconfig_base64` |

### Workflows

- **`.github/workflows/e2e-k3s.yml`** — Runs E2E tests on PRs touching k8s modules or deployers
- **`.github/workflows/e2e-health.yml`** — Daily health check (catches VPS issues before they block PRs)

## RBAC

The kubeconfig uses a scoped ServiceAccount (`e2e-runner`) — not cluster-admin. Permissions:

- Create/delete namespaces
- Full access to workload resources (pods, deployments, services, configmaps, secrets, jobs, PVCs)
- Read-only access to nodes (for health checks)
- No access to kube-system secrets or cluster configuration

## VPS Maintenance

The persistent VPS needs minimal maintenance:

| Concern | Solution |
|---------|----------|
| OS patching | `unattended-upgrades` enabled via cloud-init |
| Image GC | k3s built-in GC + `crictl rmi --prune` cron |
| Log rotation | journald `vacuum-time=7d` weekly cron |
| k3s version | Pinned (Phase 1 checks version, no auto-update) |
| Cert rotation | k3s certs valid ~1 year. Run `k3s certificate rotate` when Phase 0 auth fails |
| Full reset | `terraform destroy && terraform apply` (~10 min) |

## Troubleshooting

### Kubeconfig auth failure

k3s client certs expire after ~1 year. The SA token can be rotated independently.

```bash
# Check from the VPS
ssh ubuntu@<HOST_IP> sudo k3s kubectl get nodes

# If k3s certs expired
ssh ubuntu@<HOST_IP> sudo k3s certificate rotate
ssh ubuntu@<HOST_IP> sudo systemctl restart k3s
```

### VPS unreachable

```bash
# Check EC2 status
cd tests/e2e-k3s/infra
terraform plan -var-file=e2e.tfvars  # Shows if instance exists

# Nuke and repave
terraform destroy -var-file=e2e.tfvars
terraform apply -var-file=e2e.tfvars
# Re-extract kubeconfig (see Infrastructure Setup step 2)
```

### Image pull rate limiting

Docker Hub (busybox, curlimages/curl) has rate limits. If tests fail on image pulls:

- Wait 1 hour for rate limit reset
- Use `ghcr.io` mirrors where available
- Consider pre-pulling test images on the VPS

### DNS resolution failures

Phase 5 DNS test uses a Job (not `kubectl run --rm -i`) for reliability. If it still fails:

```bash
# Check CoreDNS is running
kubectl get pods -n kube-system -l k8s-app=kube-dns

# Check CoreDNS logs
kubectl logs -n kube-system -l k8s-app=kube-dns --tail=20

# Test manually
kubectl run dns-debug --rm -it --image=busybox:1.36 -- nslookup google.com
```

### Stale namespaces

The cleanup trap reaps namespaces prefixed with `e2e-` that are older than 30 minutes. To manually clean:

```bash
kubectl get ns | grep e2e-
kubectl delete ns <stale-namespace>
```

## Cost

| Component | Monthly | Notes |
|-----------|---------|-------|
| t3.small EC2 | ~$15 | 2 vCPU, 2GB RAM |
| EBS 30GB gp3 | ~$2.40 | Root volume |
| **Total** | **~$17/mo** | No ClickHouse cost |

**Future optimizations:** t4g.small (ARM, ~$12/mo) or spot instances (~$5-7/mo).
