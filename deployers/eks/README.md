# EKS External Deployer Reference

This folder provides Helm + ArgoCD reference assets for deploying `rpc-proxy` and `indexer` when Terraform owns only Layer 1.

## Contents

- `../charts/rpc-proxy` Helm chart (shared with k3s deployer)
- `../charts/indexer` Helm chart (shared with k3s deployer)
- `values/{dev,staging,prod}` environment overlays
- `argocd/` AppProject + dev Applications
- `scripts/render-values-from-handoff.sh`
- `scripts/populate-values-from-config-bundle.sh`

## Usage

1. Export Terraform handoff output:

```bash
terraform output -json workload_handoff > /tmp/workload_handoff.json
```

2. Render starter values from handoff:

```bash
deployers/eks/scripts/render-values-from-handoff.sh /tmp/workload_handoff.json deployers/eks/values/dev
```

3. Populate rendered values from your config bundle:

```bash
deployers/eks/scripts/populate-values-from-config-bundle.sh \
	--values-dir deployers/eks/values/dev \
	--config-dir examples/minimal_external_eks_byo/config
```

4. Review rendered values and set remaining runtime fields (URLs/secrets references).

5. Apply ArgoCD manifests:

```bash
kubectl apply -f deployers/eks/argocd/appproject.yaml
kubectl apply -f deployers/eks/argocd/application-rpc-proxy-dev.yaml
kubectl apply -f deployers/eks/argocd/application-indexer-dev.yaml
```

## Notes

- `indexer` defaults to `replicas: 1` and `Recreate` strategy for single-writer safety.
- Keep image references immutable (tag+digest preferred).
