project_name                    = "evm-cloud-smoke"
infrastructure_provider         = "aws"
deployment_target               = "managed"
runtime_arch                    = "multi"
database_mode                   = "managed"
streaming_mode                  = "disabled"
ingress_mode                    = "none"
aws_region                      = "us-east-1"
aws_skip_credentials_validation = false
networking_enabled              = true
network_environment             = "production"
network_vpc_cidr                = "10.42.0.0/16"
network_availability_zones      = ["us-east-1a", "us-east-1b"]
network_enable_nat_gateway      = false
network_enable_vpc_endpoints    = false

# Tier 0 pipeline — disabled for credential-less smoke testing.
# ECS community module requires real AWS STS access (data.aws_caller_identity).
# Enable these in a real AWS account for full pipeline plan validation.
postgres_enabled  = false
rpc_proxy_enabled = false
indexer_enabled   = false
