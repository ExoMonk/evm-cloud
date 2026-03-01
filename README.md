# evm-cloud

Automated EVM blockchain infrastructure deployment with a provider-abstraction backbone.

## What this project deploys

`evm-cloud` is an infra-first Terraform module for EVM indexing stacks on AWS:

- Networking (VPC, subnets, SGs)
- Compute substrate (`ec2` with Docker Compose, or `eks` with Kubernetes)
- Managed Postgres (optional) or BYODB ClickHouse wiring
- Runtime config channels (cloud-init + bind mounts for EC2, ConfigMap/Secret for EKS)
- eRPC + rindexer workload paths (when `workload_mode = "terraform"`)

## Layer model

`evm-cloud` supports workload ownership modes:

- `workload_mode = "terraform"` (default)
  - Terraform manages workloads (EC2 + Docker Compose services, or K8s deployments)
- `workload_mode = "external"`
  - Terraform provisions Layer 1 infra and outputs `workload_handoff` v1
  - Workloads are expected to be deployed by external tooling (CI/GitOps)

## Repository organization

- `examples/` runnable Terraform examples
- `modules/` provider adapters and infra/workload modules
- `deployers/` reference deployment scripts (EKS GitOps)
- `tests/` LocalStack harness + fixtures
- `runbooks/` operational guides
- root module (`main.tf`, `variables.tf`, `outputs.tf`) for consumers

## Examples

- `examples/minimal_aws_rds/` — EC2 + Docker Compose + managed Postgres
- `examples/minimal_aws_byo_clickhouse/` — EC2 + Docker Compose + external ClickHouse
- `examples/aws_eks_BYO_clickhouse/` — EKS + external ClickHouse
- `examples/minimal_aws_external_ec2_byo/` — EC2 external mode + BYO ClickHouse (handoff-only)
- `examples/minimal_aws_external_eks_byo/` — EKS external mode + BYO ClickHouse (handoff-only)

Each example exposes:

- `provider_selection`
- `capability_contract`
- service outputs (`postgres`, `rpc_proxy`, `indexer` when applicable)
- `workload_handoff` (v1)

## Prerequisites

- Terraform `>= 1.14.6`
- `tflint`
- `checkov`
- Docker + Docker Compose

## Quick start

```bash
cd examples/minimal_aws_byo_clickhouse

# Set up secrets
cp secrets.auto.tfvars.example secrets.auto.tfvars
# Edit: indexer_clickhouse_password, ssh_public_key

terraform init
terraform plan -var-file=minimal_clickhouse.tfvars
terraform apply -var-file=minimal_clickhouse.tfvars

# SSH into the instance
ssh -i ~/.ssh/your-key ec2-user@<public-ip>

# Check containers
sudo docker compose -f /opt/evm-cloud/docker-compose.yml ps
```

## QA and verification

Run static checks:

```bash
make qa
```

Plan one example against LocalStack:

```bash
make plan EXAMPLE=minimal_aws_rds
make plan EXAMPLE=minimal_aws_byo_clickhouse
make plan EXAMPLE=aws_eks_BYO_clickhouse
make plan EXAMPLE=minimal_aws_external_ec2_byo
make plan EXAMPLE=minimal_aws_external_eks_byo
```

Run all checks + all examples:

```bash
make verify
```

## Secrets

Each example includes `secrets.auto.tfvars.example`.

```bash
cd examples/minimal_aws_byo_clickhouse
cp secrets.auto.tfvars.example secrets.auto.tfvars
# edit values (ssh_public_key, passwords)
```

`secrets.auto.tfvars` is gitignored and auto-loaded.

## External mode quick check

```bash
cd examples/minimal_aws_byo_clickhouse
terraform init -backend=false
terraform plan -var-file=minimal_clickhouse.tfvars -var 'workload_mode=external'
```

Expected:

- infra created (networking, EC2 instance, IAM, Secrets Manager) but no Docker Compose services started
- `workload_handoff.mode = "external"`
- `workload_handoff.version = "v1"`

## Deployment runbook

For production workflow (remote state, apply order, rollback, destroy safety):

- `runbooks/aws-production-apply.md`
