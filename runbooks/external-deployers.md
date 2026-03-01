# External Deployer Runbook

This runbook describes Layer-2 deployment when Terraform is configured with:

- `workload_mode = "external"`

## 1) Provision Layer 1

Run Terraform apply for your chosen example or root module config.

Validate output:

```bash
terraform output workload_handoff
terraform output -json workload_handoff > /tmp/workload_handoff.json
```

Confirm:

- `version = "v1"`
- `mode = "external"`
- `compute_engine` matches your path (`ec2` or `eks`)

## 2) Choose deployer path

### EKS path (GitOps)

- Render values from handoff
- Commit values changes
- Apply ArgoCD AppProject/Applications
- Track sync and health in ArgoCD

### EC2 path (SSH)

```bash
PUBLIC_IP=$(terraform output -json workload_handoff | jq -r '.runtime.ec2.public_ip')
SSH_KEY=~/.ssh/your-key

# Update config files
scp -i $SSH_KEY config/erpc.yaml ec2-user@$PUBLIC_IP:/opt/evm-cloud/config/erpc.yaml
scp -i $SSH_KEY config/rindexer.yaml ec2-user@$PUBLIC_IP:/opt/evm-cloud/config/rindexer.yaml
scp -i $SSH_KEY config/abis/*.json ec2-user@$PUBLIC_IP:/opt/evm-cloud/config/abis/

# SCP docker-compose.yml (see examples/minimal_aws_external_ec2_byo/docker-compose.yml for reference)
scp -i $SSH_KEY docker-compose.yml ec2-user@$PUBLIC_IP:/opt/evm-cloud/docker-compose.yml

# Pull secrets and start services
ssh -i $SSH_KEY ec2-user@$PUBLIC_IP 'bash /opt/evm-cloud/scripts/pull-secrets.sh'
ssh -i $SSH_KEY ec2-user@$PUBLIC_IP 'cd /opt/evm-cloud && sudo docker compose --env-file .env up -d'

# Check container status
ssh -i $SSH_KEY ec2-user@$PUBLIC_IP 'sudo docker compose -f /opt/evm-cloud/docker-compose.yml ps'

# View logs
ssh -i $SSH_KEY ec2-user@$PUBLIC_IP 'sudo docker compose -f /opt/evm-cloud/docker-compose.yml logs -f'
```

## 3) Mandatory safety gates

Before promoting indexer changes:

1. **Schema compatibility**: run schema check command and fail on mismatch.
2. **Single writer**: keep desired count at 1 and avoid overlapping active indexers for the same dataset.
3. **Config immutability**: publish and record `CONFIG_BUNDLE_HASH` with the release.
4. **Resume continuity**: compare pre/post synced block and fail if post < pre.

## 4) Rollback

### EKS rollback

- Revert Helm values (image/config hash) in git
- Sync ArgoCD to previous revision
- Verify pod health and indexing progression

### EC2 rollback

- SCP previous config files back to `/opt/evm-cloud/config/`
- Restart Docker Compose services
- Verify container health and indexing progression

## 5) Operational notes

- Keep Terraform and deployer concerns separate: infra changes via Terraform, runtime release via deployers.
- Do not bypass `workload_handoff`; treat it as source-of-truth contract.
- EC2 config updates go via SSH; Terraform's `lifecycle { ignore_changes = [user_data] }` ensures no instance recreation.
