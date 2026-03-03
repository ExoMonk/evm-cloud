# k3s + Cloudflare ingress example
project_name = "evm-cloud-k3s-cf"
aws_region   = "us-east-1"

# Networking
network_vpc_cidr           = "10.42.0.0/16"
network_availability_zones = ["us-east-1a", "us-east-1b"]

# k3s host
k3s_instance_type = "t3.small"
k3s_version       = "v1.30.4+k3s1"

# Workloads
rpc_proxy_enabled = true
indexer_enabled   = true

# ClickHouse BYODB
indexer_clickhouse_user = "default"
indexer_clickhouse_db   = "rindexer"
