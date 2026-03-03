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

# Ingress — Cloudflare domain (cert/key go in secrets.auto.tfvars)
ingress_domain = "rpc.example.com"

# Sensitive values go in secrets.auto.tfvars:
#   ssh_public_key, k3s_ssh_private_key_path, k3s_api_allowed_cidrs,
#   ingress_cloudflare_origin_cert, ingress_cloudflare_origin_key,
#   indexer_clickhouse_password, indexer_clickhouse_url
