# Multi-node k3s example: server (live indexer + eRPC) + 1 worker (backfill indexer)
project_name = "evm-cloud-k3s-multi"
aws_region   = "us-east-1"

# Networking
network_vpc_cidr           = "10.42.0.0/16"
network_availability_zones = ["us-east-1a", "us-east-1b"]

# k3s server node — runs control plane + live indexer + eRPC
k3s_instance_type = "t3.small"
k3s_version       = "v1.30.4+k3s1"

# k3s worker node — spot instance for backfill indexer (~70% cheaper, interruptible)
k3s_worker_nodes = [
  { name = "backfill", role = "indexer", instance_type = "t3.small", use_spot = true },
]

# Workloads (deployed via deployers/k3s/deploy.sh after terraform apply)
rpc_proxy_enabled = true
indexer_enabled   = true

# Multi-instance indexer: live on server, backfill on worker node (spot)
# config/rindexer.yaml → live indexer (no config_key = default)
# config/backfill/rindexer.yaml → backfill indexer (config_key = "backfill")
indexer_instances = [
  { name = "indexer", node_role = "server" },
  { name = "backfill", config_key = "backfill", node_role = "indexer" },
]

# ClickHouse BYODB
indexer_clickhouse_user = "default"
indexer_clickhouse_db   = "rindexer"

# Sensitive values go in secrets.auto.tfvars:
#   ssh_public_key, k3s_ssh_private_key_path,
#   indexer_clickhouse_password, indexer_clickhouse_url
