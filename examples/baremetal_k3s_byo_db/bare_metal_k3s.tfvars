# Bare metal k3s: single-node k3s on existing VPS, PostgreSQL BYODB, inline secrets
project_name = "evm-cloud-bm-k3s"

# k3s
k3s_version = "v1.30.4+k3s1"

# Workloads (deployed via deployers/k3s/deploy.sh after terraform apply)
rpc_proxy_enabled = true
indexer_enabled   = true

# Sensitive values in secrets.auto.tfvars:
#   bare_metal_host, ssh_private_key_path, indexer_postgres_url
