# Minimal external ECS + BYO ClickHouse example
# Layer 1 (Terraform) only. Workloads are deployed by external deployers.
project_name                    = "evm-cloud-external-ecs"
infrastructure_provider         = "aws"
deployment_target               = "managed"
runtime_arch                    = "multi"
database_mode                   = "self_hosted"
streaming_mode                  = "disabled"
ingress_mode                    = "self_hosted"
compute_engine                  = "ecs"
workload_mode                   = "external"
aws_region                      = "us-east-1"
aws_skip_credentials_validation = true

networking_enabled           = true
network_environment          = "dev"
network_vpc_cidr             = "10.42.0.0/16"
network_availability_zones   = ["us-east-1a", "us-east-1b"]
network_enable_nat_gateway   = false
network_enable_vpc_endpoints = false

rpc_proxy_enabled = true
rpc_proxy_image   = "ghcr.io/erpc/erpc:latest"

indexer_enabled = true
indexer_image   = "ghcr.io/joshstevens19/rindexer:latest"

indexer_clickhouse_url  = "http://localhost:8123"
indexer_clickhouse_user = "default"
indexer_clickhouse_db   = "rindexer-test"
