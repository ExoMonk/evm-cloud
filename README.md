# evm-cloud

Automated EVM blockchain infrastructure deployment with a provider-abstraction backbone.

## Local Dev + Test Strategy

Use a 3-lane test pyramid:

1. Static IaC quality (`fmt`, `validate`, `tflint`, `checkov`)
2. Local cloud simulation (LocalStack)
3. Real AWS smoke deploy (sandbox account)

This repository now includes:

- `Makefile` with local/smoke targets
- `examples/minimal/` concrete Terraform example
- `tests/localstack/docker-compose.yml` for LocalStack runtime
- `tests/fixtures/localstack.tfvars` and `tests/fixtures/aws-smoke.tfvars`

## Repository Organization

- `examples/` user-facing runnable Terraform examples
- `tests/` test harness assets (LocalStack, fixtures, smoke helpers)
- root module files (`main.tf`, `variables.tf`, `outputs.tf`) as reusable source module

## Prerequisites

- Terraform `>= 1.14.6`
- `tflint`
- `checkov`
- Docker + Docker Compose

Check prerequisites:

```bash
make preflight
```

## QA Lane (always run)

```bash
make qa
```

## Local Simulation Lane

LocalStack compose lives in `tests/localstack/docker-compose.yml`.

Start/stop LocalStack:

```bash
make localstack-up
make localstack-logs
make localstack-down
```

Run local contract plan/apply/destroy:

```bash
make local-plan
make local-apply
make local-destroy
```

Run the concrete minimal example:

```bash
cd examples/minimal
terraform init
terraform plan -var-file=example.tfvars
```

## AWS Smoke Lane

Use a sandbox AWS account/profile and run:

```bash
make aws-smoke-plan AWS_PROFILE=your-sandbox-profile AWS_REGION=us-east-1
make aws-smoke-apply AWS_PROFILE=your-sandbox-profile AWS_REGION=us-east-1
make aws-smoke-destroy AWS_PROFILE=your-sandbox-profile AWS_REGION=us-east-1
```

## Current Scope Note

At current implementation stage (provider abstraction scaffold), these lanes validate deployment contracts and guardrails. As Tier 0 modules are added, the same commands become full infrastructure smoke tests.

## Validate

From repo root:

```bash
make qa
```
