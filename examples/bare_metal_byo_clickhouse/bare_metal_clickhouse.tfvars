# Bare metal + Docker Compose + ClickHouse BYODB
project_name = "evm-cloud-bare-metal"

# SSH connection — set real values in secrets.auto.tfvars
# bare_metal_host                 = "203.0.113.10"
# bare_metal_ssh_private_key_path = "~/.ssh/id_ed25519"
bare_metal_ssh_user = "ubuntu"
bare_metal_ssh_port = 22

# Resource limits
bare_metal_rpc_proxy_mem_limit = "500m"
bare_metal_indexer_mem_limit   = "1g"

# RPC Proxy
rpc_proxy_enabled = true
rpc_proxy_image   = "ghcr.io/erpc/erpc:latest"

# Indexer with ClickHouse BYODB
indexer_enabled = true
indexer_image   = "ghcr.io/joshstevens19/rindexer:latest"

# ClickHouse connection
indexer_clickhouse_url  = "https://your-clickhouse:8443"
indexer_clickhouse_user = "default"
indexer_clickhouse_db   = "rindexer"

workload_mode = "terraform"
