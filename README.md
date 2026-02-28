# evm-cloud

Automated EVM blockchain infrastructure deployment with a provider-abstraction backbone.

## Local Dev + Test Strategy

Use a 3-lane test pyramid:

1. Static IaC quality (`fmt`, `validate`, `tflint`, `checkov`)
2. Local cloud simulation (LocalStack)
3. Real AWS deploy (sandbox account)

This repository includes:

- `Makefile` with `plan` and `verify` targets
- `examples/minimal_rds/` — full Tier 0 pipeline with managed RDS PostgreSQL
- `examples/minimal_BYO_clickhouse/` — Tier 0 pipeline with external ClickHouse (BYODB)
- `tests/localstack/docker-compose.yml` for LocalStack runtime

## Repository Organization

- `examples/` user-facing runnable Terraform examples
- `tests/` test harness assets (LocalStack, fixtures)
- root module files (`main.tf`, `variables.tf`, `outputs.tf`) as reusable source module
- `modules/` provider adapters, networking, database, compute, and config modules

## Prerequisites

- Terraform `>= 1.14.6`
- `tflint`
- `checkov`
- Docker + Docker Compose

## QA (always run)

```bash
make qa
```

Runs `fmt-check`, `validate`, `lint`, `security` (checkov).

## Plan an Example

Plan an example against LocalStack (starts/stops LocalStack automatically):

```bash
make plan EXAMPLE=minimal_rds
make plan EXAMPLE=minimal_BYO_clickhouse
```

Default is `minimal_rds`, so bare `make plan` works.

## Secrets

Each example has a `secrets.auto.tfvars.example` template. Copy it and fill in real values:

```bash
cd examples/minimal_rds
cp secrets.auto.tfvars.example secrets.auto.tfvars
# Edit secrets.auto.tfvars with your RPC URL, etc.
```

`secrets.auto.tfvars` is gitignored and auto-loaded by Terraform.

## Full Verification

Run QA + plan all examples:

```bash
make verify
```

## Deployment Runbook

For real AWS deployment process (state backend, secrets handling, apply workflow, rollback, and destroy safety), see:

- `runbooks/aws-production-apply.md`
