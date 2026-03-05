# evm-cloud

**Deploy production EVM indexing infrastructure in minutes, not weeks.**

[![rindexer](https://img.shields.io/badge/powered%20by-rindexer-e44d26?style=flat-square)](https://rindexer.xyz) [![Terraform](https://img.shields.io/badge/Terraform-%235835CC?style=flat-square&logo=terraform&logoColor=white)](https://www.terraform.io) [![AWS](https://img.shields.io/badge/AWS-%23FF9900?style=flat-square&logo=amazon-web-services&logoColor=white)](https://aws.amazon.com) [![Docker](https://img.shields.io/badge/Docker-%232496ED?style=flat-square&logo=docker&logoColor=white)](https://www.docker.com)

<p align="center">
  <img src="assets/demo.gif" alt="evm-cloud demo" width="960" />
</p>

Every team indexing EVM data rebuilds the same stack: RPC proxy, indexer, database, networking, secrets, monitoring. Weeks of glue code and DevOps before writing a single query.

**evm-cloud** is an open-source platform that deploys the entire stack with a single command. You bring your contracts and ABIs — evm-cloud handles the infrastructure.

```bash
brew install ExoMonk/tap/evm-cloud
evm-cloud init        # interactive wizard: pick chain, database, compute
evm-cloud deploy      # provisions infra + deploys workloads
```

> **[Documentation](https://evm-cloud.xyz)** | **[Architecture](https://evm-cloud.xyz/docs/architecture)** | **[Getting Started](https://evm-cloud.xyz/docs/getting-started)** | **[Examples](https://evm-cloud.xyz/docs/examples)**

## What Gets Deployed

```
[Your contracts + ABIs]
  -> evm-cloud apply
    -> VPC + networking (security groups, subnets)
    -> Compute (EC2, EKS, k3s, or bare metal)
    -> eRPC (multi-upstream RPC proxy, failover, caching)
    -> rindexer (EVM event indexer, no-code YAML config)
    -> Database (PostgreSQL, ClickHouse — managed or BYO)
    -> Secrets management (AWS Secrets Manager, inline, or ESO)
    -> Monitoring (Prometheus + Grafana)
    -> TLS termination (Caddy or ALB)
    -> [SOON] EVM Node
    -> [SOON] Your services (APIs, ...)
```

## Who Is This For

- **Solo builders** shipping on-chain apps who don't want to manage infra
- **Data teams** building analytics dashboards on decoded blockchain events
- **Protocol teams** needing real-time event monitoring and alerting
- **Anyone** tired of stitching together 5+ tools just to index contract events

## Quick Start

```bash
# Install
brew install ExoMonk/tap/evm-cloud

# Initialize a new project
evm-cloud init

# Deploy everything
evm-cloud apply

# Tear down when done
evm-cloud destroy --yes
```

Or try it locally first (no cloud account needed):

```bash
evm-cloud local up    # kind cluster + Anvil + eRPC + rindexer + ClickHouse
```

See the [Getting Started guide](https://evm-cloud.xyz/docs/getting-started) for the full walkthrough.

## Deployment Patterns

| Pattern | Compute | Database | Starting At |
|---------|---------|----------|-------------|
| [`baremetal_byo_clickhouse`](examples/baremetal_byo_clickhouse/) | Docker Compose (any VPS) | ClickHouse (BYO) | ~$5/mo |
| [`minimal_aws_byo_clickhouse`](examples/minimal_aws_byo_clickhouse/) | EC2 + Docker | ClickHouse (BYO) | ~$35/mo |
| [`minimal_aws_k3s_byo_clickhouse`](examples/minimal_aws_k3s_byo_clickhouse/) | k3s (lightweight K8s) | ClickHouse (BYO) | ~$35/mo |
| [`minimal_aws_rds`](examples/minimal_aws_rds/) | EC2 + Docker | Managed PostgreSQL (RDS) | ~$45/mo |
| [`prod_aws_k3s_multi_byo_clickhouse`](examples/prod_aws_k3s_multi_byo_clickhouse/) | k3s multi-node + Secrets Manager + ESO | ClickHouse (BYO) | ~$40/mo |
| [`aws_eks_BYO_clickhouse`](examples/aws_eks_BYO_clickhouse/) | EKS (managed Kubernetes) | ClickHouse (BYO) | ~$110/mo |
| [`baremetal_k3s_byo_db`](examples/baremetal_k3s_byo_db/) | Bare metal k3s (any VPS) | PostgreSQL (BYO) | Your VPS |
| [`minimal_aws_external_ec2_byo`](examples/minimal_aws_external_ec2_byo/) | EC2 (infra only, BYO deployer) | ClickHouse (BYO) | ~$35/mo |

See [Choosing a Pattern](https://evm-cloud.xyz/docs/examples) for help picking the right one.

## Install

### Homebrew (recommended)

```bash
brew install ExoMonk/tap/evm-cloud
```

### curl (GitHub Releases)

```bash
curl -fsSL https://github.com/ExoMonk/evm-cloud/releases/latest/download/install.sh | bash
```

### Pinned version

```bash
curl -fsSL https://github.com/ExoMonk/evm-cloud/releases/download/0.0.1-alpha4/install.sh | bash -s -- 0.0.1-alpha4
```

### Source build

```bash
git clone https://github.com/ExoMonk/evm-cloud.git && cd evm-cloud/cli
cargo install --path .
```

### CI (GitHub Actions)

```yaml
- name: Install evm-cloud
  run: curl -fsSL https://github.com/ExoMonk/evm-cloud/releases/download/0.0.1-alpha4/install.sh | bash -s -- 0.0.1-alpha4
```

## Architecture

evm-cloud is a modular Terraform platform with a Rust CLI orchestrator. The CLI wraps Terraform + deployer scripts into a unified workflow.

```
evm-cloud init → scaffolds Terraform config from evm-cloud.toml
evm-cloud apply → terraform apply + workload deployment
evm-cloud deploy → re-deploy workloads without re-running Terraform
evm-cloud destroy → teardown everything
evm-cloud local → local dev stack (kind + Anvil)
```

11 Terraform modules, 4 compute engines (EC2, EKS, k3s, bare metal), 2 database backends, secrets management, monitoring, and TLS — all composable.

See the full [Architecture docs](https://evm-cloud.xyz/docs/architecture) for how the modules fit together.

## Documentation

Full docs at **[evm-cloud.xyz](https://evm-cloud.xyz)**:

- [Getting Started](https://evm-cloud.xyz/docs/getting-started) — deploy your first indexer
- [Architecture](https://evm-cloud.xyz/docs/architecture) — how modules fit together
- [Core Concepts](https://evm-cloud.xyz/docs/concepts) — providers, compute engines, workload modes
- [CLI Reference](https://evm-cloud.xyz/docs/cli-reference) — all commands and flags
- [Variable Reference](https://evm-cloud.xyz/docs/variable-reference) — configuration options + sizing guide
- [Cost Estimates](https://evm-cloud.xyz/docs/cost-estimates) — what each pattern costs
- [Guides](https://evm-cloud.xyz/docs/guides) — secrets, TLS, config updates, production checklist
- [Troubleshooting](https://evm-cloud.xyz/docs/troubleshooting) — common issues and fixes

## Prerequisites

- Terraform >= 1.5.0
- AWS CLI v2 (for AWS deployments)
- SSH key pair
- `jq` (for k3s/EKS deployers)

## Contributing

```bash
make qa          # fmt, validate, lint, security (checkov)
make verify      # qa + plan all examples
make test-k8s    # Kubernetes chart tests (kind)
```

## License

Apache 2.0
