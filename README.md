# 🏰 evm-cloud

[![rindexer](https://img.shields.io/badge/powered%20by-rindexer-e44d26?style=flat-square)](https://rindexer.xyz) [![Terraform](https://img.shields.io/badge/Terraform-%235835CC?style=flat-square&logo=terraform&logoColor=white)](https://www.terraform.io) [![AWS](https://img.shields.io/badge/AWS-%23FF9900?style=flat-square&logo=amazon-web-services&logoColor=white)](https://aws.amazon.com) [![Docker](https://img.shields.io/badge/Docker-%232496ED?style=flat-square&logo=docker&logoColor=white)](https://www.docker.com)

> **🚧 Work in Progress** — Actively building, expect breaking changes.

Open-source data infrastructure platform for EVM blockchain data. Deploy, manage, and scale a complete data stack — Nodes, RPC proxies, data indexers, databases, and networking — on AWS or bare metal with a single `terraform apply`.

> **[Architecture](https://evm-cloud.xyz/docs/architecture)** | [Getting Started](https://evm-cloud.xyz/docs/getting-started) | [Examples](https://evm-cloud.xyz/docs/examples)

## What It Deploys

- **eRPC** — multi-upstream RPC proxy with automatic failover and caching
- **rindexer** — EVM event indexer (no-code YAML config)
- **Database** — managed PostgreSQL (RDS) or bring-your-own ClickHouse
- **Networking** — VPC, subnets, security groups (AWS)
- **Compute** — EC2 + Docker Compose, EKS, k3s, or bare metal

## Install CLI (Standalone)

Use this if you want to run `evm-cloud` from your own Terraform repository (without cloning this repo).

### Option 1: Homebrew (recommended)

```bash
brew install ExoMonk/tap/evm-cloud
evm-cloud --help
```

### Option 2: curl installer (GitHub Releases)

Install latest release:

```bash
curl -fsSL https://github.com/ExoMonk/evm-cloud/releases/latest/download/install.sh | bash
evm-cloud --help
```

Install a pinned version:

```bash
curl -fsSL https://github.com/ExoMonk/evm-cloud/releases/download/v0.1.0/install.sh | bash -s -- v0.1.0
```

### Option 3: Source build

```bash
git clone https://github.com/ExoMonk/evm-cloud.git
cd evm-cloud/cli
cargo install --path .
evm-cloud --help
```

## Quick Start

```bash
cd examples/minimal_aws_byo_clickhouse

# Configure secrets
cp secrets.auto.tfvars.example secrets.auto.tfvars
# Edit: ssh_public_key, indexer_clickhouse_password, indexer_clickhouse_url

terraform init
terraform plan -var-file=minimal_clickhouse.tfvars
terraform apply -var-file=minimal_clickhouse.tfvars
```

See [Getting Started](https://evm-cloud.xyz/docs/getting-started) for the full walkthrough.

## Examples

| Example | Compute | Database | Cost |
|---------|---------|----------|------|
| [`minimal_aws_rds`](examples/minimal_aws_rds/) | EC2 + Docker | Managed PostgreSQL | ~$45/mo |
| [`minimal_aws_byo_clickhouse`](examples/minimal_aws_byo_clickhouse/) | EC2 + Docker | ClickHouse (BYODB) | ~$35/mo |
| [`aws_eks_BYO_clickhouse`](examples/aws_eks_BYO_clickhouse/) | EKS (Kubernetes) | ClickHouse (BYODB) | ~$110/mo |
| [`minimal_aws_k3s_byo_clickhouse`](examples/minimal_aws_k3s_byo_clickhouse/) | k3s (lightweight K8s) | ClickHouse (BYODB) | ~$35/mo |
| [`prod_aws_k3s_multi_byo_clickhouse`](examples/prod_aws_k3s_multi_byo_clickhouse/) | k3s multi-node (server + spot worker) + SM + ESO | ClickHouse (BYODB) | ~$40/mo |
| [`baremetal_byo_clickhouse`](examples/baremetal_byo_clickhouse/) | Docker Compose (any VPS) | ClickHouse (BYODB) | ~$5-20/mo |
| [`minimal_aws_external_ec2_byo`](examples/minimal_aws_external_ec2_byo/) | EC2 (infra only) | ClickHouse (BYODB) | ~$35/mo |
| [`baremetal_k3s_byo_db`](examples/baremetal_k3s_byo_db/) | Bare metal k3s (any VPS) | PostgreSQL (BYODB) | Your VPS |

See [Choosing an Example](https://evm-cloud.xyz/docs/examples) for help picking the right one.

## Prerequisites

- Terraform >= 1.5.0
- AWS CLI v2 (for AWS deployments)
- SSH key pair
- `jq` (for k3s/EKS external deployers)

## QA and Verification

```bash
make qa          # fmt, validate, lint, security (checkov)
make verify      # qa + plan all examples
make test-k8s    # Kubernetes chart tests (kind)
```

## Terraform CLI Migration (CLI1.1)

The new Rust CLI wraps the existing Terraform flow while preserving an escape hatch for direct Terraform usage.

Recommended entrypoint from repo root:

```bash
make evm-cloud
```

This prints CLI help by default.

If you installed the standalone binary, use `evm-cloud` directly.

Use this flag routing rule:
- `evm-cloud` flags go first (for example: `--dir`, `--allow-raw-terraform`)
- Terraform passthrough flags go after a second `--` (for example: `-var-file`, `-parallelism`)

Canonical example:

```bash
make evm-cloud apply -- --dir examples/baremetal_k3s_byo_db --allow-raw-terraform -- -var-file=bare_metal_k3s.tfvars -parallelism=3
```

| Existing workflow | New wrapper |
|---|---|
| `terraform init` | `evm-cloud init` |
| `terraform apply` | `evm-cloud apply` |
| `terraform destroy` | `evm-cloud destroy --yes` |
| `terraform apply -parallelism=50` | `evm-cloud apply -- -- -parallelism=50` |

Safety defaults:
- `destroy` requires explicit `--yes`.
- In non-interactive shells, `destroy` requires both `--yes` and `--auto-approve`.
- In non-interactive shells, `apply` requires `--auto-approve`.
- Raw Terraform roots require explicit `--allow-raw-terraform`.

Current scope note:
- `init`, `apply`, and `destroy` are functional wrappers.
- `deploy`, `status`, and `logs` are scaffolded and currently print "not yet implemented".

Raw Terraform remains supported for advanced workflows and troubleshooting.

## Documentation

Full documentation lives in [`documentation/`](https://evm-cloud.xyz/docs):

- [Architecture](https://evm-cloud.xyz/docs/architecture) — how the modules fit together
- [Core Concepts](https://evm-cloud.xyz/docs/concepts) — providers, compute engines, workload modes
- [Variable Reference](https://evm-cloud.xyz/docs/variable-reference) — all configuration options with sizing guide
- [Cost Estimates](https://evm-cloud.xyz/docs/cost-estimates) — what each pattern costs
- [Guides](https://evm-cloud.xyz/docs/guides) — secrets, config updates, production checklist
- [Troubleshooting](https://evm-cloud.xyz/docs/troubleshooting) — common issues and fixes

## License

Apache 2.0
