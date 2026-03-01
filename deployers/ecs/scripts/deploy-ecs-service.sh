#!/usr/bin/env bash
set -euo pipefail

if ! command -v aws >/dev/null 2>&1; then
  echo "aws CLI is required" >&2
  exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required" >&2
  exit 1
fi

usage() {
  cat <<EOF
Usage:
  $0 --handoff-file <path> --service <rpc-proxy|indexer> --taskdef <path>

For indexer deployments, required env vars:
  CONFIG_BUNDLE_HASH
  SCHEMA_CHECK_CMD
  PRE_CHECK_CMD
  POST_CHECK_CMD

Optional env vars:
  AWS_REGION_OVERRIDE
EOF
}

HANDOFF_FILE=""
SERVICE=""
TASKDEF_FILE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --handoff-file) HANDOFF_FILE="$2"; shift 2 ;;
    --service) SERVICE="$2"; shift 2 ;;
    --taskdef) TASKDEF_FILE="$2"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; usage; exit 1 ;;
  esac
done

if [[ -z "$HANDOFF_FILE" || -z "$SERVICE" || -z "$TASKDEF_FILE" ]]; then
  usage
  exit 1
fi

MODE=$(jq -r '.mode' "$HANDOFF_FILE")
ENGINE=$(jq -r '.compute_engine' "$HANDOFF_FILE")
CLUSTER_ARN=$(jq -r '.runtime.ecs.cluster_arn // empty' "$HANDOFF_FILE")
PROJECT=$(jq -r '.project_name' "$HANDOFF_FILE")
REGION=$(jq -r '.aws_region' "$HANDOFF_FILE")
REGION="${AWS_REGION_OVERRIDE:-$REGION}"

if [[ "$MODE" != "external" || "$ENGINE" != "ecs" ]]; then
  echo "handoff must be mode=external and compute_engine=ecs" >&2
  exit 1
fi

if [[ -z "$CLUSTER_ARN" ]]; then
  echo "runtime.ecs.cluster_arn missing from handoff" >&2
  exit 1
fi

SERVICE_NAME=""
RPC_PROXY_DISCOVERY_ARN=""
case "$SERVICE" in
  rpc-proxy)
    SERVICE_NAME=$(jq -r '.services.rpc_proxy.service_name // empty' "$HANDOFF_FILE")
    RPC_PROXY_DISCOVERY_ARN=$(jq -r '.services.rpc_proxy.discovery.service_arn // empty' "$HANDOFF_FILE")
    ;;
  indexer)
    SERVICE_NAME=$(jq -r '.services.indexer.service_name // empty' "$HANDOFF_FILE")
    ;;
  *)
    echo "service must be rpc-proxy or indexer" >&2
    exit 1
    ;;
esac

if [[ -z "$SERVICE_NAME" ]]; then
  echo "service_name missing in handoff for $SERVICE" >&2
  exit 1
fi

SUBNETS_JSON=$(jq -c '.network.private_subnet_ids // []' "$HANDOFF_FILE")
if [[ "$SERVICE" == "rpc-proxy" ]]; then
  SERVICE_SG=$(jq -r '.network.security_groups.rpc_proxy // empty' "$HANDOFF_FILE")
else
  SERVICE_SG=$(jq -r '.network.security_groups.indexer // empty' "$HANDOFF_FILE")
fi

if [[ "$SUBNETS_JSON" == "[]" || -z "$SERVICE_SG" ]]; then
  echo "Missing private subnet IDs or service security group in workload_handoff.network" >&2
  exit 1
fi

PRE_BLOCK=""
if [[ "$SERVICE" == "indexer" ]]; then
  : "${CONFIG_BUNDLE_HASH:?CONFIG_BUNDLE_HASH is required for indexer deploys}"
  : "${SCHEMA_CHECK_CMD:?SCHEMA_CHECK_CMD is required for indexer deploys}"
  : "${PRE_CHECK_CMD:?PRE_CHECK_CMD is required for indexer deploys}"
  : "${POST_CHECK_CMD:?POST_CHECK_CMD is required for indexer deploys}"

  echo "Running schema compatibility gate..."
  bash --noprofile --norc -lc "$SCHEMA_CHECK_CMD"

  echo "Running pre-deploy checkpoint gate..."
  PRE_BLOCK=$(bash --noprofile --norc -lc "$PRE_CHECK_CMD")
  if [[ ! "$PRE_BLOCK" =~ ^[0-9]+$ ]]; then
    echo "PRE_CHECK_CMD must output an integer block number" >&2
    exit 1
  fi
fi

echo "Registering task definition from $TASKDEF_FILE"

LOG_GROUP_NAME=$(jq -r '.containerDefinitions[0].logConfiguration.options["awslogs-group"] // empty' "$TASKDEF_FILE")
if [[ -n "$LOG_GROUP_NAME" ]]; then
  LOG_GROUP_EXISTS=$(aws logs describe-log-groups \
    --region "$REGION" \
    --log-group-name-prefix "$LOG_GROUP_NAME" \
    | jq -r --arg lg "$LOG_GROUP_NAME" '[.logGroups[]? | select(.logGroupName == $lg)] | length')

  if [[ "$LOG_GROUP_EXISTS" == "0" ]]; then
    echo "Creating missing CloudWatch log group: $LOG_GROUP_NAME"
    aws logs create-log-group \
      --region "$REGION" \
      --log-group-name "$LOG_GROUP_NAME" >/dev/null

    aws logs put-retention-policy \
      --region "$REGION" \
      --log-group-name "$LOG_GROUP_NAME" \
      --retention-in-days 30 >/dev/null || true
  fi
fi

TD_ARN=$(aws ecs register-task-definition \
  --region "$REGION" \
  --cli-input-json "file://$TASKDEF_FILE" \
  | jq -r '.taskDefinition.taskDefinitionArn')

SERVICE_EXISTS=$(aws ecs describe-services \
  --region "$REGION" \
  --cluster "$CLUSTER_ARN" \
  --services "$SERVICE_NAME" \
  | jq -r '.services | length')

if [[ "$SERVICE_EXISTS" == "0" ]]; then
  echo "Service $SERVICE_NAME not found. Creating it..."
  CREATE_JSON=$(mktemp)
  jq -n \
    --arg service_name "$SERVICE_NAME" \
    --arg cluster "$CLUSTER_ARN" \
    --arg task_def "$TD_ARN" \
    --argjson subnets "$SUBNETS_JSON" \
    --arg sg "$SERVICE_SG" \
    --arg desired "$([[ "$SERVICE" == "indexer" ]] && echo 1 || echo 1)" \
    '{
      serviceName: $service_name,
      cluster: $cluster,
      taskDefinition: $task_def,
      desiredCount: ($desired | tonumber),
      launchType: "FARGATE",
      deploymentConfiguration: {
        maximumPercent: 200,
        minimumHealthyPercent: 0
      },
      networkConfiguration: {
        awsvpcConfiguration: {
          subnets: $subnets,
          securityGroups: [$sg],
          assignPublicIp: "DISABLED"
        }
      }
    }' > "$CREATE_JSON"

  if [[ "$SERVICE" == "rpc-proxy" && -n "$RPC_PROXY_DISCOVERY_ARN" ]]; then
    jq --arg registry_arn "$RPC_PROXY_DISCOVERY_ARN" '
      .serviceRegistries = [{
        registryArn: $registry_arn
      }]
    ' "$CREATE_JSON" > "$CREATE_JSON.tmp" && mv "$CREATE_JSON.tmp" "$CREATE_JSON"
  fi

  aws ecs create-service \
    --region "$REGION" \
    --cli-input-json "file://$CREATE_JSON" >/dev/null

  rm -f "$CREATE_JSON"
else
  echo "Updating existing service $SERVICE_NAME on cluster $CLUSTER_ARN"
  if [[ "$SERVICE" == "indexer" ]]; then
    aws ecs update-service \
      --region "$REGION" \
      --cluster "$CLUSTER_ARN" \
      --service "$SERVICE_NAME" \
      --task-definition "$TD_ARN" \
      --desired-count 1 \
      --force-new-deployment >/dev/null
  else
    aws ecs update-service \
      --region "$REGION" \
      --cluster "$CLUSTER_ARN" \
      --service "$SERVICE_NAME" \
      --task-definition "$TD_ARN" \
      --force-new-deployment >/dev/null
  fi
fi

aws ecs wait services-stable \
  --region "$REGION" \
  --cluster "$CLUSTER_ARN" \
  --services "$SERVICE_NAME"

echo "Deployment completed: $TD_ARN"

if [[ "$SERVICE" == "indexer" ]]; then
  echo "Running post-deploy continuity gate..."
  POST_BLOCK=$(bash --noprofile --norc -lc "$POST_CHECK_CMD")
  if [[ ! "$POST_BLOCK" =~ ^[0-9]+$ ]]; then
    echo "POST_CHECK_CMD must output an integer block number" >&2
    exit 1
  fi
  if (( POST_BLOCK < PRE_BLOCK )); then
    echo "Continuity gate failed: post-deploy block ($POST_BLOCK) < pre-deploy block ($PRE_BLOCK)" >&2
    exit 1
  fi
  echo "Continuity gate passed: $PRE_BLOCK -> $POST_BLOCK"
fi
