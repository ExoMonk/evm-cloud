# evm-cloud

Automated EVM blockchain infrastructure deployment with a provider-abstraction backbone.

## What this project deploys

`evm-cloud` is an infra-first Terraform module for EVM indexing stacks on AWS:

- Networking (VPC, subnets, SGs)
- Compute substrate (`ecs` or `eks`)
- Managed Postgres (optional) or BYODB clickhouse wiring
- Runtime config channels (S3 for ECS, ConfigMap/Secret for EKS)
- eRPC + rindexer workload paths (when `workload_mode = "terraform"`)

## Layer model (Phase A/A.1)

`evm-cloud` supports workload ownership modes:

- `workload_mode = "terraform"` (default)
  - Terraform manages workloads (ECS services or K8s deployments)
- `workload_mode = "external"`
  - Terraform provisions Layer 1 infra and outputs `workload_handoff` v1
  - Workloads are expected to be deployed by external tooling (CI/GitOps)

Phase A.1 adds Layer-1 ECS IAM roles so external deployers can use stable role ARNs from `workload_handoff.identity`.

## Repository organization

- `examples/` runnable Terraform examples
- `modules/` provider adapters and infra/workload modules
- `tests/` LocalStack harness + fixtures
- root module (`main.tf`, `variables.tf`, `outputs.tf`) for consumers

## Examples

- `examples/minimal_rds/` — ECS + managed Postgres
- `examples/minimal_BYO_clickhouse/` — ECS + external ClickHouse
- `examples/eks_BYO_clickhouse/` — EKS + external ClickHouse
- `examples/minimal_external_ecs_byo/` — ECS external mode + BYO ClickHouse (handoff-only)
- `examples/minimal_external_eks_byo/` — EKS external mode + BYO ClickHouse (handoff-only)

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

## QA and verification

Run static checks:

```bash
make qa
```

Plan one example against LocalStack:

```bash
make plan EXAMPLE=minimal_rds
make plan EXAMPLE=minimal_BYO_clickhouse
make plan EXAMPLE=eks_BYO_clickhouse
make plan EXAMPLE=minimal_external_ecs_byo
make plan EXAMPLE=minimal_external_eks_byo
```

Run all checks + all examples:

```bash
make verify
```

## Secrets

Each example includes `secrets.auto.tfvars.example`.

```bash
cd examples/minimal_rds
cp secrets.auto.tfvars.example secrets.auto.tfvars
# edit values
```

`secrets.auto.tfvars` is gitignored and auto-loaded.

## External mode quick check

```bash
cd examples/minimal_BYO_clickhouse
terraform init -backend=false
terraform plan -var-file=minimal_clickhouse.tfvars -var 'workload_mode=external'
```

Expected:

- no workload resources (`aws_ecs_service` / k8s deployment resources)
- `workload_handoff.mode = "external"`
- `workload_handoff.version = "v1"`

## Deployment runbook

For production workflow (remote state, apply order, rollback, destroy safety):

- `runbooks/aws-production-apply.md`
