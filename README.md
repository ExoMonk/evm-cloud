# 🏰 evm-cloud

[![rindexer](https://img.shields.io/badge/powered%20by-rindexer-e44d26?style=flat-square)](https://rindexer.xyz) [![Terraform](https://img.shields.io/badge/Terraform-%235835CC?style=flat-square&logo=terraform&logoColor=white)](https://www.terraform.io) [![AWS](https://img.shields.io/badge/AWS-%23FF9900?style=flat-square&logo=amazon-web-services&logoColor=white)](https://aws.amazon.com) [![Docker](https://img.shields.io/badge/Docker-%232496ED?style=flat-square&logo=docker&logoColor=white)](https://www.docker.com)

> **🚧 Work in Progress** — Actively building, expect breaking changes.

Open-source infrastructure platform for EVM blockchain data. Deploy, manage, and scale a complete data stack — RPC proxies, event indexers, databases, and networking — on AWS or bare metal with a single `terraform apply`.

> **[Full Documentation](./documentation/docs/index.mdx)** | [Getting Started](./documentation/docs/getting-started.mdx) | [Variable Reference](./documentation/docs/variable-reference.mdx) | [Examples](./documentation/docs/examples/index.mdx)

## What It Deploys

- **eRPC** — multi-upstream RPC proxy with automatic failover and caching
- **rindexer** — EVM event indexer (no-code YAML config)
- **Database** — managed PostgreSQL (RDS) or bring-your-own ClickHouse
- **Networking** — VPC, subnets, security groups (AWS)
- **Compute** — EC2 + Docker Compose, EKS, k3s, or bare metal

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

See [Getting Started](./documentation/docs/getting-started.mdx) for the full walkthrough.

## Examples

| Example | Compute | Database | Cost |
|---------|---------|----------|------|
| [`minimal_aws_rds`](examples/minimal_aws_rds/) | EC2 + Docker | Managed PostgreSQL | ~$45/mo |
| [`minimal_aws_byo_clickhouse`](examples/minimal_aws_byo_clickhouse/) | EC2 + Docker | ClickHouse (BYODB) | ~$35/mo |
| [`aws_eks_BYO_clickhouse`](examples/aws_eks_BYO_clickhouse/) | EKS (Kubernetes) | ClickHouse (BYODB) | ~$110/mo |
| [`minimal_aws_k3s_byo_clickhouse`](examples/minimal_aws_k3s_byo_clickhouse/) | k3s (lightweight K8s) | ClickHouse (BYODB) | ~$35/mo |
| [`bare_metal_byo_clickhouse`](examples/bare_metal_byo_clickhouse/) | Docker Compose (any VPS) | ClickHouse (BYODB) | ~$5-20/mo |
| [`minimal_aws_external_ec2_byo`](examples/minimal_aws_external_ec2_byo/) | EC2 (infra only) | ClickHouse (BYODB) | ~$35/mo |
| [`minimal_aws_external_eks_byo`](examples/minimal_aws_external_eks_byo/) | EKS (infra only) | ClickHouse (BYODB) | ~$110/mo |

See [Choosing an Example](./documentation/docs/examples/index.mdx) for help picking the right one.

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

## Documentation

Full documentation lives in [`documentation/`](./documentation/docs/index.mdx):

- [Architecture](./documentation/docs/architecture.mdx) — how the modules fit together
- [Core Concepts](./documentation/docs/concepts.mdx) — providers, compute engines, workload modes
- [Variable Reference](./documentation/docs/variable-reference.mdx) — all configuration options with sizing guide
- [Cost Estimates](./documentation/docs/cost-estimates.mdx) — what each pattern costs
- [Guides](./documentation/docs/guides/secrets-management.mdx) — secrets, config updates, production checklist
- [Troubleshooting](./documentation/docs/troubleshooting.mdx) — common issues and fixes

## License

Apache 2.0
