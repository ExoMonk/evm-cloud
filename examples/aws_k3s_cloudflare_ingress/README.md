# AWS k3s + Cloudflare Ingress

Production-ready EVM indexing stack on a single EC2 instance with Cloudflare TLS termination.

**Stack:** EC2 → k3s → ingress-nginx → eRPC + rindexer → ClickHouse (BYODB)

**Cost:** ~$17/mo (t3.small) + ClickHouse Cloud free tier

## Architecture

```
Client
  → Cloudflare edge (free TLS, DDoS protection, WAF)
    → Origin cert (encrypted)
      → ingress-nginx (ports 80/443 via hostPort)
        → eRPC :4000 (ClusterIP, not exposed externally)
        → rindexer (indexes to ClickHouse)
```

Cloudflare handles edge TLS and routes traffic to your origin. The origin cert encrypts the connection between Cloudflare and your server. Port 4000 (eRPC) is never exposed directly — all traffic goes through the ingress controller.

## Prerequisites

- AWS CLI configured (`aws configure`)
- Terraform >= 1.5.0
- kubectl, helm, jq
- An SSH key pair
- A domain managed by Cloudflare
- A ClickHouse instance (e.g. [ClickHouse Cloud](https://clickhouse.cloud/) free tier)

## Setup

### 1. Cloudflare Origin Certificate

1. Go to the [Cloudflare dashboard](https://dash.cloudflare.com/) → select your domain
2. **SSL/TLS → Origin Server → Create Certificate**
   - Key type: RSA (2048)
   - Hostnames: `*.yourdomain.com, yourdomain.com`
   - Validity: 15 years
3. **Copy both the certificate and private key** — the key is shown only once
4. **SSL/TLS → Overview** → set encryption mode to **Full (strict)**

### 2. Configure secrets

```bash
cp secrets.auto.tfvars.example secrets.auto.tfvars
```

Fill in `secrets.auto.tfvars`:

```hcl
# SSH
ssh_public_key           = "ssh-ed25519 AAAA..."
k3s_ssh_private_key_path = "~/.ssh/id_ed25519"
k3s_api_allowed_cidrs    = ["YOUR_IP/32"]  # curl -s ifconfig.me

# ClickHouse BYODB
indexer_clickhouse_url      = "https://your-host:8443"
indexer_clickhouse_password = "your-password"

# Cloudflare origin certificate
ingress_domain = "rpc.yourdomain.com"

ingress_cloudflare_origin_cert = <<-EOT
-----BEGIN CERTIFICATE-----
<paste from Cloudflare>
-----END CERTIFICATE-----
EOT

ingress_cloudflare_origin_key = <<-EOT
-----BEGIN PRIVATE KEY-----
<paste from Cloudflare>
-----END PRIVATE KEY-----
EOT
```

### 3. Configure workloads (optional)

Edit the config files to match your use case:

- `config/erpc.yaml` — RPC proxy config (default: Ethereum mainnet via public RPC)
- `config/rindexer.yaml` — indexer config (default: USDC Transfer events)
- `config/abis/ERC20.json` — contract ABIs

### 4. Deploy

```bash
# Phase 1: Provision EC2 + k3s
terraform init
terraform plan -var-file=cloudflare_k3s.tfvars -out=plan.tfplan
terraform apply plan.tfplan

# Extract handoff for Phase 2
terraform output -json workload_handoff > handoff.json

# Phase 2: Deploy workloads (eRPC, rindexer, ingress-nginx, TLS secret)
../../deployers/k3s/deploy.sh handoff.json --config-dir ./config
```

### 5. Point DNS to your server

Get the EC2 public IP:

```bash
terraform output -json workload_handoff | jq -r '.runtime.k3s.host_ip'
```

In Cloudflare DNS, add an **A record**:

| Type | Name | Content | Proxy status |
|------|------|---------|-------------|
| A | `rpc` | `<EC2 public IP>` | Proxied (orange cloud) |

### 6. Verify

```bash
# Should return eRPC response through Cloudflare TLS
curl https://rpc.yourdomain.com/

# Port 4000 should NOT be accessible directly
curl http://<EC2_IP>:4000  # should timeout
```

## Files

| File | Purpose |
|------|---------|
| `main.tf` | Root module config — k3s + Cloudflare ingress |
| `variables.tf` | All input variables with defaults |
| `cloudflare_k3s.tfvars` | Non-secret config (instance type, region, etc.) |
| `secrets.auto.tfvars.example` | Template for secrets — copy to `secrets.auto.tfvars` |
| `config/erpc.yaml` | eRPC proxy configuration |
| `config/rindexer.yaml` | rindexer indexer configuration |
| `config/abis/` | Contract ABIs for rindexer |

## Customization

### Skip the indexer

If you just want the RPC proxy with TLS (no indexing):

```hcl
# In cloudflare_k3s.tfvars
indexer_enabled = false
```

The ClickHouse variables become optional.

### Use a private RPC upstream

Edit `config/erpc.yaml` and replace the public endpoint:

```yaml
upstreams:
  - id: primary
    endpoint: https://your-private-rpc.com/v1/YOUR_KEY
    type: evm
```

### Change instance size

For heavier workloads (multiple chains, high-volume indexing):

```hcl
# In cloudflare_k3s.tfvars
k3s_instance_type = "t3.medium"  # 2 vCPU, 4GB (~$30/mo)
```

## Teardown

```bash
# Remove workloads from k3s
../../deployers/k3s/teardown.sh handoff.json

# Destroy AWS infrastructure
terraform destroy -var-file=cloudflare_k3s.tfvars
```

Don't forget to remove the DNS A record in Cloudflare.
