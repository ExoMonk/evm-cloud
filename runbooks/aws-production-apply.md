# AWS Deployment Runbook

This runbook defines the required process for deploying evm-cloud in real AWS environments.

## 1) Environment Model

Use isolated environments with separate state and credentials:

- `sandbox` (daily testing)
- `staging` (pre-production validation)
- `production` (live)

Rules:

- Never share Terraform state between environments
- Prefer separate AWS accounts per environment
- Use distinct IAM roles/profiles per environment

## 2) Prerequisites

- Terraform `>= 1.14.6`
- `make`, `tflint`, `checkov`
- Docker + Docker Compose
- AWS credentials configured (`AWS_PROFILE`)

Baseline check:

```bash
make preflight
make qa
```

## 3) Remote State Backend

Use an S3 backend + DynamoDB lock table for each environment.

Required backend controls:

- S3 versioning enabled
- Bucket encryption enabled (SSE-KMS preferred)
- Public access blocked
- DynamoDB table for state locking

Example backend file (`backends/production.hcl`):

```hcl
bucket         = "evm-cloud-terraform-state-prod"
key            = "core/terraform.tfstate"
region         = "us-east-1"
dynamodb_table = "evm-cloud-terraform-locks-prod"
encrypt        = true
```

Initialize:

```bash
terraform init -reconfigure -backend-config=backends/production.hcl
```

## 4) Secrets & Sensitive Data

Never commit secrets to git or `.tfvars` in repository paths.

Allowed secret sources:

- AWS Secrets Manager
- AWS SSM Parameter Store
- CI secret injection via `TF_VAR_*`

Example:

```bash
export TF_VAR_project_name="evm-cloud-prod"
```

## 5) Pre-Deploy Validation Gate

All of the following must pass before any apply:

```bash
make qa
make local-plan
make aws-smoke-plan AWS_PROFILE=<profile> AWS_REGION=<region>
cd examples/minimal
terraform init -backend=false
terraform validate
terraform plan -var-file=example.tfvars
cd ../..
```

If any step fails, stop and fix before deploy.

## 6) Deployment Procedure (Apply)

### 6.1 Build reviewed plan artifact

```bash
terraform plan -var-file=tests/fixtures/aws-smoke.tfvars -out=.terraform/prod.plan
```

### 6.2 Review plan

- Confirm only expected resources/changes
- Confirm no destructive actions unless explicitly approved

### 6.3 Apply approved plan

```bash
terraform apply .terraform/prod.plan
```

Always apply saved plan artifacts; avoid implicit plan+apply in production.

## 7) Post-Apply Verification

Run immediate verification after apply:

- `terraform output`
- CloudWatch log/metric checks
- endpoint health checks (when applicable)
- smoke query checks against deployed services

Capture results in deployment notes.

## 8) Rollback Procedure

Rollback is controlled and plan-based:

1. Identify last known-good git revision
2. Generate rollback plan from that revision
3. Validate rollback plan in staging (preferred)
4. Apply rollback plan in production during approved change window

For data-bearing systems, prefer forward fixes unless restore procedures are explicitly approved.

## 9) Destroy Safety Policy

`terraform destroy` is forbidden in production without explicit approval.

Minimum controls:

- Separate IAM role for destroy operations
- MFA / break-glass approval workflow
- Environment guard variable for non-prod destroy only

Sandbox destroy example:

```bash
make aws-smoke-destroy AWS_PROFILE=<profile> AWS_REGION=<region>
```

## 10) CI/CD Gate

Required on every PR:

- `make qa`
- `make local-plan`
- `make aws-smoke-plan`
- `examples/minimal` init/validate/plan

Recommended on main/nightly:

- sandbox apply
- smoke verification
- optional sandbox destroy (ephemeral policy)
