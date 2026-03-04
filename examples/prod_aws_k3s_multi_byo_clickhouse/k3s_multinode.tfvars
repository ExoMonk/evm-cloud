# Production multi-node k3s: server + spot worker, secrets via AWS Secrets Manager + ESO
project_name = "evm-cloud-k3s-prod"
aws_region   = "us-east-1"

# Networking
network_vpc_cidr           = "10.42.0.0/16"
network_availability_zones = ["us-east-1a", "us-east-1b"]

# k3s server node — runs control plane + live indexer + eRPC + Monitoring stack
k3s_instance_type = "c7i-flex.large"
k3s_version       = "v1.30.4+k3s1"

# k3s worker node — spot instance for backfill indexer (~70% cheaper, interruptible)
k3s_worker_nodes = [
  { name = "backfill", role = "indexer", instance_type = "t3.micro", use_spot = true },
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

# Secrets — provider mode (AWS SM + ESO)
# ClickHouse password stored in Secrets Manager, synced to K8s via ESO.
# No passwords in the handoff JSON or Helm values.
secrets_mode                       = "provider"
ec2_secret_recovery_window_in_days = 0 # Immediate deletion for dev/test

# Sensitive values go in secrets.auto.tfvars:
#   ssh_public_key, k3s_ssh_private_key_path,
#   indexer_clickhouse_password, indexer_clickhouse_url
# Optional: secrets_manager_secret_arn (BYOA — skip SM secret creation)
