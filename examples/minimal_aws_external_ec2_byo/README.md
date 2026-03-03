# External EC2 + BYO ClickHouse Example

## What this is

Terraform provisions the full infrastructure (VPC, IAM, EC2 instance, Secrets Manager, CloudWatch) but does **not** deploy workloads (no config files, no `docker compose up`). The instance boots with Docker + Compose installed and ready. External tooling (CI/CD, deployer scripts, Ansible) handles the application layer.

## What Gets Deployed (Terraform)

- VPC (`10.42.0.0/16`) with DNS support
- 2 public + 2 private subnets across `us-east-1a` and `us-east-1b`
- Internet Gateway + route tables
- Security Groups (ec2: SSH + eRPC, indexer, erpc)
- IAM instance role + profile (CloudWatch Logs + Secrets Manager)
- EC2 instance (Docker + Compose pre-installed via cloud-init)
- Secrets Manager secret (ClickHouse credentials)
- CloudWatch Log Group

## What Does NOT Get Deployed (Deployer responsibility)

- No docker-compose.yml written
- No erpc.yaml / rindexer.yaml config files
- No ABI files
- No `docker compose up` — containers are not started

## Usage

```bash
cd examples/minimal_aws_external_ec2_byo

# Set up secrets
cp secrets.auto.tfvars.example secrets.auto.tfvars
# Edit: indexer_clickhouse_password, ssh_public_key

terraform init
terraform plan -var-file=minimal_external_ec2_byo.tfvars
terraform apply -var-file=minimal_external_ec2_byo.tfvars

# Get handoff output for your deployer
terraform output -json workload_handoff
```

## Deployer Workflow

After `terraform apply`, the EC2 instance is running with Docker ready. Use the compose deployer:

```bash
# Initial deploy + config updates (idempotent)
terraform output -json workload_handoff | ../../deployers/compose/deploy.sh /dev/stdin \
  --config-dir ./config \
  --ssh-key ~/.ssh/id_
```

The script uploads configs via SCP and runs `docker compose up --force-recreate`. Re-run the same command after editing config files to push updates.

### Manual workflow (alternative)

```bash
# 1. Get connection info from handoff
PUBLIC_IP=$(terraform output -json workload_handoff | jq -r '.runtime.ec2.public_ip')
SSH_KEY=~/.ssh/your-key

# 2. SCP config files
scp -i $SSH_KEY config/erpc.yaml ec2-user@$PUBLIC_IP:/opt/evm-cloud/config/erpc.yaml
scp -i $SSH_KEY config/rindexer.yaml ec2-user@$PUBLIC_IP:/opt/evm-cloud/config/rindexer.yaml
scp -i $SSH_KEY config/abis/*.json ec2-user@$PUBLIC_IP:/opt/evm-cloud/config/abis/

# 3. SCP docker-compose.yml (edit the reference file in this example first)
scp -i $SSH_KEY config/docker-compose.yml ec2-user@$PUBLIC_IP:/opt/evm-cloud/docker-compose.yml

# 4. Pull secrets and start services
ssh -i $SSH_KEY ec2-user@$PUBLIC_IP 'bash /opt/evm-cloud/scripts/pull-secrets.sh'
ssh -i $SSH_KEY ec2-user@$PUBLIC_IP 'cd /opt/evm-cloud && sudo docker compose --env-file .env up -d'
```

## workload_handoff v1 Output

```hcl
workload_handoff = {
  version        = "v1"
  mode           = "external"
  compute_engine = "ec2"

  identity = {
    ec2_instance_profile = { name = "...", role_arn = "..." }
  }

  network = {
    vpc_id             = "vpc-..."
    public_subnet_ids  = ["subnet-...", "subnet-..."]
    security_groups    = { rpc_proxy = "sg-...", indexer = "sg-..." }
  }

  runtime = {
    ec2 = {
      instance_id  = "i-..."       # Instance exists, ready for workloads
      public_ip    = "3.x.x.x"
      ssh_command  = "ssh -i ... ec2-user@3.x.x.x"
      config_dir   = "/opt/evm-cloud/config"
      compose_file = "/opt/evm-cloud/docker-compose.yml"
      secret_arn   = "arn:aws:secretsmanager:..."
    }
  }

  services = {
    rpc_proxy = { service_name = "erpc", port = 4000 }
    indexer   = { service_name = "rindexer", storage_backend = "clickhouse" }
  }

  data = {
    backend    = "clickhouse"
    clickhouse = { url = "http://clickhouse.example.com:8123", user = "default", db = "rindexer" }
  }
}
```

## Differences from terraform-managed example

| | `minimal_aws_byo_clickhouse` | `minimal_aws_external_ec2_byo` |
|---|---|---|
| **workload_mode** | `terraform` | `external` |
| **EC2 instance** | Created by Terraform | Created by Terraform |
| **Docker Compose** | cloud-init writes + starts | You SCP + start |
| **Config files** | Baked into cloud-init | You deploy via SSH |
| **Secrets** | Auto-pulled on boot | You run `pull-secrets.sh` |
| **Use case** | Dev / quick start | CI/CD / GitOps / custom deploy |
