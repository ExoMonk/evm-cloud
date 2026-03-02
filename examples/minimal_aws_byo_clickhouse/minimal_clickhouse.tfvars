# Minimal ClickHouse example: eRPC + rindexer with BYODB ClickHouse
project_name                    = "evm-cloud-minimal-clickhouse"
infrastructure_provider         = "aws"
deployment_target               = "managed"
runtime_arch                    = "multi"
database_mode                   = "self_hosted"
streaming_mode                  = "disabled"
ingress_mode                    = "self_hosted"
aws_region                      = "us-east-1"
aws_skip_credentials_validation = false

# Networking
networking_enabled           = true
network_environment          = "dev"
network_vpc_cidr             = "10.42.0.0/16"
network_availability_zones   = ["us-east-1a", "us-east-1b"]
network_enable_nat_gateway   = true
network_enable_vpc_endpoints = true

# No managed Postgres — using external ClickHouse

# RPC Proxy
rpc_proxy_enabled = true
rpc_proxy_image   = "ghcr.io/erpc/erpc:latest"

# Indexer with ClickHouse BYODB
indexer_enabled = true
indexer_image   = "ghcr.io/joshstevens19/rindexer:latest"
# indexer_rpc_url → auto-resolves to http://erpc:4000 (Docker Compose service name) when rpc_proxy_enabled=true

# ClickHouse connection (Bring Your Own Database)
# indexer_clickhouse_url → set in secrets.auto.tfvars (contains hostname)
indexer_clickhouse_user = "default"
indexer_clickhouse_db   = "rindexer"

# EC2+Docker Compose compute engine
compute_engine    = "ec2"
ec2_instance_type = "t3.micro"
# ssh_public_key → set in secrets.auto.tfvars (ssh-rsa AAAA... or ssh-ed25519 AAAA...)

# Config files: erpc.yaml, rindexer.yaml, and abis/ are read via file()
# in main.tf from files alongside this example. Edit those files directly.
workload_mode = "terraform"
