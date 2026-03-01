# Minimal External EKS + BYO ClickHouse Example

This example provisions Layer 1 infrastructure only and emits `workload_handoff` for external deployers.

## Mode

- `compute_engine = "eks"`
- `workload_mode = "external"`

Terraform provisions:

- VPC/subnets/security groups
- EKS cluster substrate
- IAM foundations for EKS workloads

Terraform does **not** provision Kubernetes workloads in this example.

## Usage

### 1) Provision Layer 1 with Terraform

```bash
# Move into this example
cd examples/minimal_external_eks_byo

# Copy secrets template and fill in real values
cp secrets.auto.tfvars.example secrets.auto.tfvars
# Edit secrets.auto.tfvars:
# - indexer_rpc_url
# - indexer_clickhouse_password

terraform init
terraform plan -var-file=minimal_external_eks_byo.tfvars
terraform apply -var-file=minimal_external_eks_byo.tfvars
```

### 2) Export handoff for external deployers

```bash
terraform output -json workload_handoff > /tmp/workload_handoff.json
```

### 3) Configure kubectl from handoff

```bash
CLUSTER_NAME="$(jq -r '.runtime.eks.cluster_name' /tmp/workload_handoff.json)"
AWS_REGION="$(jq -r '.aws_region' /tmp/workload_handoff.json)"

aws eks update-kubeconfig --name "$CLUSTER_NAME" --region "$AWS_REGION"
```

### 4) Render starter values from handoff

```bash
# Run from repository root
cd ../..

OUT_DIR=/tmp/evm-cloud-eks-values
deployers/eks/scripts/render-values-from-handoff.sh /tmp/workload_handoff.json "$OUT_DIR"
```

### 5) Fill runtime config in rendered values

Use the example config bundle and then edit any remaining values:


```bash
deployers/eks/scripts/populate-values-from-config-bundle.sh \
  --values-dir "$OUT_DIR" \
  --config-dir examples/minimal_external_eks_byo/config
```

Then set in `$OUT_DIR/indexer-values.yaml`:

- `clickhouse.password`
- `rpcUrl` (for in-cluster rpc-proxy)

```bash
RPC_SERVICE_NAME="$(jq -r '.services.rpc_proxy.service_name' /tmp/workload_handoff.json)"
echo "Use this rpcUrl in indexer-values.yaml: http://${RPC_SERVICE_NAME}.default.svc.cluster.local:4000"
```

### 6) Deploy with Helm reference charts

```bash
helm upgrade --install evm-cloud-rpc-proxy \
  deployers/eks/charts/rpc-proxy \
  -f "$OUT_DIR/rpc-proxy-values.yaml" \
  --namespace default --create-namespace

helm upgrade --install evm-cloud-indexer \
  deployers/eks/charts/indexer \
  -f "$OUT_DIR/indexer-values.yaml" \
  --namespace default --create-namespace
```

### 7) Optional checks

```bash
kubectl get deploy,svc,pods
kubectl logs -l app.kubernetes.io/name=indexer --tail=200
```

## Expected output shape

`workload_handoff` includes:

- `mode = "external"`
- `compute_engine = "eks"`
- EKS runtime fields under `runtime.eks.*`
- service metadata under `services.*`
- storage backend metadata under `data.*`
