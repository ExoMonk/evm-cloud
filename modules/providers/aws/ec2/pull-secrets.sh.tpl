#!/bin/bash
set -euo pipefail

SECRET_ID="${secret_id}"
REGION="${aws_region}"
ENV_FILE="/opt/evm-cloud/.env"

echo "Pulling secrets from Secrets Manager: $SECRET_ID"
aws secretsmanager get-secret-value \
  --secret-id "$SECRET_ID" \
  --region "$REGION" \
  --query SecretString \
  --output text | jq -r 'to_entries[] | "\(.key)=\(.value)"' > "$ENV_FILE"

chmod 600 "$ENV_FILE"
echo "Secrets written to $ENV_FILE"
