# External Deployer Runbook (Phase B)

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
- `compute_engine` matches your path (`ecs` or `eks`)

## 2) Choose deployer path

### EKS path (GitOps)

- Render values from handoff
- Commit values changes
- Apply ArgoCD AppProject/Applications
- Track sync and health in ArgoCD

### ECS path (CI)

- Render task definition from handoff
- Register task definition revision
- Update ECS service
- Wait for stable service state

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

### ECS rollback

- Identify previous stable task definition revision
- Update service to previous revision
- Verify service steady state and continuity gate

## 5) Operational notes

- Keep Terraform and deployer concerns separate: infra changes via Terraform, runtime release via deployers.
- Do not bypass `workload_handoff`; treat it as source-of-truth contract.
