# External Deployers (Phase B)

Reference Layer-2 deployment assets for `workload_mode = "external"`.

## Structure

- `compose/` SSH-based Docker Compose deployment (EC2, VPS, bare metal)
- `eks/` Helm + ArgoCD GitOps references
- `k3s/` Helm-based deployment for k3s clusters

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
