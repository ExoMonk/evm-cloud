# Indexes Uniswap V4 Swap events on Ethereum + Base → ClickHouse, serves via custom API service
# Bare metal k3s — works on any VPS (Hetzner, OVH, DigitalOcean, etc.)
project_name = "defi-swaps"

# k3s
k3s_version = "v1.30.4+k3s1"

# ClickHouse BYODB
indexer_clickhouse_user = "default"
indexer_clickhouse_db   = "rindexer"

# Sensitive values in secrets.auto.tfvars:
#   bare_metal_host, ssh_private_key_path,
#   indexer_clickhouse_password, indexer_clickhouse_url,
#   webhook_secret
