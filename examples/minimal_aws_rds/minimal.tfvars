# Minimal example: full Tier 0 pipeline on LocalStack
project_name                    = "evm-cloud-example"
infrastructure_provider         = "aws"
deployment_target               = "managed"
runtime_arch                    = "multi"
database_mode                   = "managed"
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

# Postgres
postgres_enabled          = true
postgres_instance_class   = "db.t4g.micro"
postgres_engine_version   = "16.4"
postgres_db_name          = "rindexer"
postgres_db_username      = "rindexer"
postgres_backup_retention = 7

# RPC Proxy
rpc_proxy_enabled = true
rpc_proxy_image   = "ghcr.io/erpc/erpc:latest"

# Indexer
indexer_enabled         = true
indexer_image           = "ghcr.io/joshstevens19/rindexer:latest"
indexer_storage_backend = "postgres"
# indexer_rpc_url → auto-resolves to http://erpc:4000 when rpc_proxy_enabled=true

# EC2+Docker Compose compute engine
compute_engine = "ec2"
# ssh_public_key → set in secrets.auto.tfvars

# Config files: erpc.yaml, rindexer.yaml, and abis/ are read via file()
# in main.tf from files alongside this example. Edit those files directly.
workload_mode = "terraform"
