# External Deployers (Phase B)

Reference Layer-2 deployment assets for `workload_mode = "external"`.

## Structure

- `eks/` Helm + ArgoCD GitOps references
- `ecs/` ECS task-definition + CI deployment references

## Contract source

All deployers consume Terraform output:

- `output.workload_handoff` (v1)

Do not hardcode cluster names, role ARNs, subnet IDs, or security groups.

## Safety gates

Reference flows enforce:

- Single-writer indexer invariant
- Config bundle hash requirement
- Resume continuity pre/post checks
- Schema compatibility check hook
