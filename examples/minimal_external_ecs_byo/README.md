# Minimal External ECS + BYO ClickHouse Example

This example provisions Layer 1 infrastructure only and emits `workload_handoff` for external deployers.

## Mode

- `compute_engine = "ecs"`
- `workload_mode = "external"`

Terraform provisions:

- VPC/subnets/security groups
- ECS cluster
- Layer-1 ECS IAM roles for rpc-proxy/indexer
- S3 config bucket (artifact channel)

Terraform does **not** provision ECS services/task definitions in this example.

## Usage

### 1) Provision Layer 1 with Terraform

```bash
# Move into this example
cd examples/minimal_external_ecs_byo

# Copy secrets template and fill in real values
cp secrets.auto.tfvars.example secrets.auto.tfvars
# Edit secrets.auto.tfvars:
# - indexer_rpc_url
# - indexer_clickhouse_password

terraform init
terraform plan -var-file=minimal_external_ecs_byo.tfvars -var 'aws_skip_credentials_validation=false'
terraform apply -var-file=minimal_external_ecs_byo.tfvars -var 'aws_skip_credentials_validation=false'

# IMPORTANT: keep `network_environment` consistent with existing state.
# If your state already uses `production`, applying with `network_environment=dev`
# will plan replacement of core resources (VPC/ECS cluster/S3).
# Example (production):
# terraform apply -var-file=minimal_external_ecs_byo.tfvars \
#   -var 'network_environment=production' \
#   -var 'network_enable_nat_gateway=true' \
#   -var 'aws_skip_credentials_validation=false'
```

### 2) Export handoff for external deployers

```bash
terraform output -json workload_handoff > /tmp/workload_handoff.json
```

### 3) Upload runtime config bundle to S3 artifact channel

```bash
# Run from repository root
cd ../..

deployers/ecs/scripts/upload-config-from-handoff.sh \
	--handoff-file /tmp/workload_handoff.json \
	--config-dir examples/minimal_external_ecs_byo/config
```

### 4) Create ClickHouse password secret for ECS indexer

```bash
CLICKHOUSE_SECRET_ARN="$(deployers/ecs/scripts/upsert-clickhouse-secret.sh \
	--handoff-file /tmp/workload_handoff.json \
	--secret-value 'REPLACE_ME')"

echo "Using ClickHouse secret ARN: $CLICKHOUSE_SECRET_ARN"
```

### 5) Render and deploy `rpc-proxy`

```bash
# Run from repository root
cd ../..

CONFIG_BUNDLE_HASH=$(git rev-parse --short HEAD) \
deployers/ecs/scripts/render-taskdef-from-handoff.sh \
	--handoff-file /tmp/workload_handoff.json \
	--service rpc-proxy \
	--image ghcr.io/erpc/erpc:latest \
	--out /tmp/rpc-proxy.json

# If templates changed during troubleshooting, always re-render taskdefs
# before deploy. The current templates use a sidecar to sync config from S3.

deployers/ecs/scripts/deploy-ecs-service.sh \
	--handoff-file /tmp/workload_handoff.json \
	--service rpc-proxy \
	--taskdef /tmp/rpc-proxy.json
```

### 6) Verify Cloud Map endpoint is healthy (before indexer)

```bash
RPC_INTERNAL_URL="$(jq -r '.services.rpc_proxy.discovery.internal_url' /tmp/workload_handoff.json)"

# Resolve DNS and run a quick health probe from inside the VPC
# (for example: bastion host, SSM shell, or ECS exec). If this does not
# resolve/reply yet, wait a few seconds and retry.
echo "$RPC_INTERNAL_URL"
curl -sS --max-time 5 "$RPC_INTERNAL_URL" >/dev/null && echo "rpc-proxy reachable"
```

### 7) Render and deploy `indexer` (ClickHouse backend)

```bash
# RPC_URL is auto-derived from Cloud Map handoff (`services.rpc_proxy.discovery.internal_urterraform destroy -var-file=minimal_clickhouse.tfvars.l`)
# when omitted. This gives a stable internal endpoint instead of task IP pinning.
# You can still override RPC_URL explicitly if needed.
CONFIG_BUNDLE_HASH=$(git rev-parse --short HEAD) \
CLICKHOUSE_PASSWORD_SECRET_ARN="$CLICKHOUSE_SECRET_ARN" \
deployers/ecs/scripts/render-taskdef-from-handoff.sh \
	--handoff-file /tmp/workload_handoff.json \
	--service indexer \
	--image ghcr.io/joshstevens19/rindexer:latest \
	--out /tmp/indexer.json

# Optional override example:
# CONFIG_BUNDLE_HASH=$(git rev-parse --short HEAD) \
# RPC_URL='http://custom-erpc-endpoint:4000' \
# CLICKHOUSE_PASSWORD_SECRET_ARN="$CLICKHOUSE_SECRET_ARN" \
# deployers/ecs/scripts/render-taskdef-from-handoff.sh ...

CONFIG_BUNDLE_HASH=$(git rev-parse --short HEAD) \
SCHEMA_CHECK_CMD='echo 1' \
PRE_CHECK_CMD='echo 100' \
POST_CHECK_CMD='echo 101' \
deployers/ecs/scripts/deploy-ecs-service.sh \
	--handoff-file /tmp/workload_handoff.json \
	--service indexer \
	--taskdef /tmp/indexer.json
```

### 8) Optional checks

```bash
aws ecs list-services \
	--cluster "$(jq -r '.runtime.ecs.cluster_arn' /tmp/workload_handoff.json)" \
	--region "$(jq -r '.aws_region' /tmp/workload_handoff.json)"
```

The deployer scripts used above are:

- `deployers/ecs/scripts/upload-config-from-handoff.sh`
- `deployers/ecs/scripts/upsert-clickhouse-secret.sh`
- `deployers/ecs/scripts/render-taskdef-from-handoff.sh`
- `deployers/ecs/scripts/deploy-ecs-service.sh`

## Expected output shape

`workload_handoff` includes:

- `mode = "external"`
- `compute_engine = "ecs"`
- ECS identity ARNs under `identity.ecs_*`
- ECS cluster under `runtime.ecs.cluster_arn`
- service names under `services.*.service_name`
- S3 artifact locations under `artifacts.s3.*`
