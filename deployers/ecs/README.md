# ECS External Deployer Reference

This folder provides CI-friendly ECS deployment references for `workload_mode = "external"`.

## Contents

- `task-defs/*.tpl.json` task definition templates
- `scripts/upload-config-from-handoff.sh`
- `scripts/upsert-clickhouse-secret.sh`
- `scripts/render-taskdef-from-handoff.sh`
- `scripts/deploy-ecs-service.sh`

## Required inputs

- `workload_handoff` JSON (from Terraform output)
- immutable image reference
- `CONFIG_BUNDLE_HASH`

For indexer deployment gates:

- `SCHEMA_CHECK_CMD`
- `PRE_CHECK_CMD`
- `POST_CHECK_CMD`

## Usage

1. Export handoff output:

```bash
terraform output -json workload_handoff > /tmp/workload_handoff.json
```

2. Upload runtime config bundle:

```bash
deployers/ecs/scripts/upload-config-from-handoff.sh \
  --handoff-file /tmp/workload_handoff.json \
  --config-dir examples/minimal_external_ecs_byo/config
```

3. Upsert ClickHouse password secret (for ClickHouse indexer path):

```bash
CLICKHOUSE_PASSWORD_SECRET_ARN="$(deployers/ecs/scripts/upsert-clickhouse-secret.sh \
  --handoff-file /tmp/workload_handoff.json \
  --secret-value 'REPLACE_ME')"
```

4. Render task definition:

```bash
CONFIG_BUNDLE_HASH=$(git rev-parse HEAD) \
deployers/ecs/scripts/render-taskdef-from-handoff.sh \
  --handoff-file /tmp/workload_handoff.json \
  --service rpc-proxy \
  --image ghcr.io/erpc/erpc:latest \
  --out /tmp/rpc-proxy-taskdef.json
```

5. Deploy:

```bash
deployers/ecs/scripts/deploy-ecs-service.sh \
  --handoff-file /tmp/workload_handoff.json \
  --service rpc-proxy \
  --taskdef /tmp/rpc-proxy-taskdef.json
```

6. Verify Cloud Map endpoint before indexer deploy:

```bash
# Run from a VPC-reachable context (bastion host, SSM shell, or ECS exec).
RPC_INTERNAL_URL="$(jq -r '.services.rpc_proxy.discovery.internal_url' /tmp/workload_handoff.json)"
echo "$RPC_INTERNAL_URL"
curl -sS --max-time 5 "$RPC_INTERNAL_URL" >/dev/null && echo "rpc-proxy reachable"
```

For indexer deployments, set the required gate commands before the indexer deploy step.
`RPC_URL` is optional for indexer render — when omitted, it is auto-derived
from `workload_handoff.services.rpc_proxy.discovery.internal_url`
(Cloud Map stable endpoint).
