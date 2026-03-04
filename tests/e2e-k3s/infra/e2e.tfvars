# E2E test infrastructure defaults
# Secrets (ssh_public_key, k3s_ssh_private_key_path, k3s_api_allowed_cidrs)
# go in secrets.auto.tfvars (gitignored)

project_name      = "evm-cloud-e2e"
aws_region        = "us-east-1"
k3s_instance_type = "c7i-flex.large"
k3s_version       = "v1.30.4+k3s1"
