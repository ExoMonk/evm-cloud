terraform {
  required_version = ">= 1.5.0"

  required_providers {
    null = {
      source  = "hashicorp/null"
      version = ">= 3.0"
    }
  }
}

# =============================================================================
# Persistent k3s VPS for E2E testing
# Provisions once, kept alive. Tests connect via kubeconfig ($0/run, ~$17/mo).
# =============================================================================

module "evm_cloud" {
  source = "../../.."

  project_name            = var.project_name
  infrastructure_provider = "aws"
  deployment_target       = "managed"
  runtime_arch            = "multi"
  database_mode           = "self_hosted"
  streaming_mode          = "disabled"
  ingress_mode            = "none"

  compute_engine = "k3s"
  workload_mode  = "external"

  # SSH keys
  ssh_public_key           = var.ssh_public_key
  k3s_ssh_private_key_path = var.k3s_ssh_private_key_path
  k3s_instance_type        = var.k3s_instance_type
  k3s_version              = var.k3s_version
  k3s_api_allowed_cidrs    = var.k3s_api_allowed_cidrs

  # AWS
  aws_region                      = var.aws_region
  aws_skip_credentials_validation = false
  networking_enabled              = true
  network_environment             = "dev"
  network_vpc_cidr                = "10.42.0.0/16"
  network_availability_zones      = ["${var.aws_region}a", "${var.aws_region}b"]
  network_enable_nat_gateway      = false
  network_enable_vpc_endpoints    = false

  # RPC Proxy — deployed via Helm by deployers/k3s/deploy.sh
  rpc_proxy_enabled = true
  rpc_proxy_image   = "ghcr.io/erpc/erpc:latest"

  # Indexer — no real ClickHouse; rindexer will CrashLoop (by design for E2E)
  indexer_enabled         = true
  indexer_image           = "ghcr.io/joshstevens19/rindexer:latest"
  indexer_rpc_url         = ""
  indexer_storage_backend = "clickhouse"

  # Fake ClickHouse credentials — rindexer CrashLoops, which is expected.
  # E2E tests validate deployer pipeline, not indexer connectivity.
  indexer_clickhouse_url      = "http://localhost:8123"
  indexer_clickhouse_user     = "default"
  indexer_clickhouse_password = "e2e-test-not-real"
  indexer_clickhouse_db       = "default"

  # Monitoring
  monitoring_enabled      = true
  grafana_ingress_enabled = false

  # Config injection
  erpc_config_yaml     = file("${path.module}/../config/erpc.yaml")
  rindexer_config_yaml = file("${path.module}/../config/rindexer.yaml")
  rindexer_abis = {
    "ERC20.json" = file("${path.module}/../config/abis/ERC20.json")
  }
}

# =============================================================================
# Post-provision: Create scoped RBAC for CI runner
# =============================================================================

resource "null_resource" "e2e_rbac" {
  depends_on = [module.evm_cloud]

  triggers = {
    kubeconfig = module.evm_cloud.workload_handoff.runtime.k3s.kubeconfig_base64
  }

  provisioner "local-exec" {
    command = <<-SCRIPT
      KUBECONFIG_FILE=$(mktemp)
      echo "${module.evm_cloud.workload_handoff.runtime.k3s.kubeconfig_base64}" | base64 -d > "$KUBECONFIG_FILE"
      chmod 0600 "$KUBECONFIG_FILE"
      export KUBECONFIG="$KUBECONFIG_FILE"

      # Apply RBAC manifests
      kubectl apply -f - <<'EOF'
      apiVersion: v1
      kind: ServiceAccount
      metadata:
        name: e2e-runner
        namespace: kube-system
      ---
      apiVersion: rbac.authorization.k8s.io/v1
      kind: ClusterRole
      metadata:
        name: e2e-runner
      rules:
        - apiGroups: [""]
          resources: ["namespaces"]
          verbs: ["create", "delete", "get", "list"]
        - apiGroups: ["", "apps", "batch"]
          resources: ["pods", "pods/log", "pods/exec", "deployments", "services",
                       "configmaps", "secrets", "persistentvolumeclaims",
                       "persistentvolumes", "jobs", "serviceaccounts"]
          verbs: ["*"]
        - apiGroups: [""]
          resources: ["nodes"]
          verbs: ["get", "list"]
        - apiGroups: ["rbac.authorization.k8s.io"]
          resources: ["roles", "rolebindings"]
          verbs: ["*"]
      ---
      apiVersion: rbac.authorization.k8s.io/v1
      kind: ClusterRoleBinding
      metadata:
        name: e2e-runner
      roleRef:
        apiGroup: rbac.authorization.k8s.io
        kind: ClusterRole
        name: e2e-runner
      subjects:
        - kind: ServiceAccount
          name: e2e-runner
          namespace: kube-system
      ---
      apiVersion: v1
      kind: Secret
      metadata:
        name: e2e-runner-token
        namespace: kube-system
        annotations:
          kubernetes.io/service-account.name: e2e-runner
      type: kubernetes.io/service-account-token
      EOF

      # Wait for token to be populated
      sleep 5

      rm -f "$KUBECONFIG_FILE"
    SCRIPT
  }
}

# =============================================================================
# Outputs
# =============================================================================

output "workload_handoff" {
  description = "Full workload handoff (cluster-admin kubeconfig)"
  value       = module.evm_cloud.workload_handoff
  sensitive   = true
}

output "host_ip" {
  description = "EC2 instance public IP"
  value       = try(module.evm_cloud.workload_handoff.runtime.k3s.cluster_endpoint, "")
  sensitive   = true
}

output "project_name" {
  value = var.project_name
}

# Scoped kubeconfig for CI — uses e2e-runner ServiceAccount token
output "e2e_kubeconfig_base64" {
  description = "Base64-encoded kubeconfig using scoped e2e-runner SA (for CI)"
  sensitive   = true
  value       = "Run: kubectl --kubeconfig=<admin-kubeconfig> get secret e2e-runner-token -n kube-system -o jsonpath='{.data.token}' | base64 -d to get the SA token, then build a kubeconfig with it. See README.md for details."
}
