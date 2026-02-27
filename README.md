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
- `modules/networking/` Tier 0 AWS networking foundation module (optional enablement via `networking_enabled`)

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

`tests/fixtures/localstack.tfvars` sets `aws_skip_credentials_validation=true` so local simulation does not require real STS credentials.

Run the concrete minimal example:

```bash
make example-plan
```

## AWS Smoke Lane

Use a sandbox AWS account/profile and run:

```bash
make aws-smoke-plan AWS_PROFILE=your-sandbox-profile AWS_REGION=us-east-1
make aws-smoke-apply AWS_PROFILE=your-sandbox-profile AWS_REGION=us-east-1
make aws-smoke-destroy AWS_PROFILE=your-sandbox-profile AWS_REGION=us-east-1
```

By default, `aws-smoke-*` runs with `AWS_SMOKE_SKIP_CREDENTIALS_VALIDATION=true` so plan checks can run in credential-less environments.

For strict real-account credential validation, set:

```bash
make aws-smoke-plan AWS_PROFILE=your-sandbox-profile AWS_REGION=us-east-1 AWS_SMOKE_SKIP_CREDENTIALS_VALIDATION=false
```

## Current Scope Note

At current implementation stage (provider abstraction scaffold), these lanes validate deployment contracts and guardrails. As Tier 0 modules are added, the same commands become full infrastructure smoke tests.

## Validate

From repo root:

```bash
make qa
```

## Deployment Runbook

For real AWS deployment process (state backend, secrets handling, apply workflow, rollback, and destroy safety), see:

- `runbooks/aws-production-apply.md`
