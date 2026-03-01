# Minimal ClickHouse example: eRPC + rindexer with BYODB ClickHouse
project_name                    = "evm-cloud-clickhouse"
infrastructure_provider         = "aws"
deployment_target               = "managed"
runtime_arch                    = "multi"
database_mode                   = "self_hosted"
streaming_mode                  = "disabled"
ingress_mode                    = "self_hosted"
aws_region                      = "us-east-1"
aws_skip_credentials_validation = true

# Networking
networking_enabled           = true
network_environment          = "dev"
network_vpc_cidr             = "10.42.0.0/16"
network_availability_zones   = ["us-east-1a", "us-east-1b"]
network_enable_nat_gateway   = false
network_enable_vpc_endpoints = false

# No managed Postgres — using external ClickHouse

# RPC Proxy
rpc_proxy_enabled = true
rpc_proxy_image   = "ghcr.io/erpc/erpc:latest"

# Indexer with ClickHouse BYODB
indexer_enabled = true
indexer_image   = "ghcr.io/joshstevens19/rindexer:latest"
# indexer_rpc_url → set in secrets.auto.tfvars

# ClickHouse connection (Bring Your Own Database)
indexer_clickhouse_url  = "http://clickhouse.example.com:8123"
indexer_clickhouse_user = "default"
indexer_clickhouse_db   = "rindexer"

# Config files: erpc.yaml, rindexer.yaml, and abis/ are read via file()
# in main.tf from files alongside this example. Edit those files directly.
workload_mode = "terraform"
