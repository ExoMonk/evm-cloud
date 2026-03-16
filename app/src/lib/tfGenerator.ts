/**
 * Terraform scaffold generator — ports cli/src/codegen/scaffold.rs + manifest.rs
 *
 * Generates: versions.tf, main.tf, variables.tf, outputs.tf,
 * terraform.auto.tfvars, secrets.auto.tfvars.example,
 * backend.tfbackend, .gitignore, Makefile, README.md
 *
 * IMPORTANT: Schema contract test (M1) verifies parity with Rust CLI output.
 */

import {
  type BuilderState,
  BUILDER_VERSION,
  BUILDER_MODULE_VERSION,
} from "./configSchema.ts";

// ---------------------------------------------------------------------------
// Variable manifest — port of cli/src/codegen/manifest.rs
// ---------------------------------------------------------------------------

type HclType = "string" | "bool" | "number" | "list(string)" | "map(string)";

type Condition =
  | "always"
  | "cloud"           // provider != bare_metal
  | "bare_metal"      // provider == bare_metal
  | "postgres"        // storage_backend includes postgres
  | "byodb_postgres"  // postgres + self_hosted
  | "clickhouse"      // storage_backend includes clickhouse
  | "managed_postgres"// postgres + managed
  | "ec2"             // engine == ec2
  | "cloud_k3s"       // cloud + engine == k3s
  | "k8s"             // engine == k3s || eks
  | "monitoring"      // k8s + monitoring enabled
  | "ingress_cloudflare"
  | "ingress_caddy"
  | "ingress_nginx"
  | "ingress_tls"     // caddy || ingress_nginx
  | "secrets_provider"
  | "secrets_external"
  | "secrets_provider_or_external";

interface VarEntry {
  name: string;
  type: HclType;
  condition: Condition;
  default?: string | null; // HCL literal, null means no default
  sensitive: boolean;
  description: string;
  group?: string; // section comment header
}

// The full manifest (117 variables, conditionally included)
const MANIFEST: VarEntry[] = [
  // ── Core ──────────────────────────────────────────────────────────────
  { name: "project_name", type: "string", condition: "always", sensitive: false, description: "Name of the deployment project", group: "Core" },
  { name: "infrastructure_provider", type: "string", condition: "always", sensitive: false, description: "Cloud provider: aws or bare_metal" },
  { name: "database_mode", type: "string", condition: "always", sensitive: false, description: "Database deployment: managed or self_hosted" },
  { name: "compute_engine", type: "string", condition: "always", sensitive: false, description: "Compute engine: ec2, eks, k3s, or docker_compose" },
  { name: "workload_mode", type: "string", condition: "always", sensitive: false, description: "Workload management: terraform or external" },
  { name: "secrets_mode", type: "string", condition: "always", sensitive: false, description: "Secrets management: inline, provider, or external" },
  { name: "ingress_mode", type: "string", condition: "always", sensitive: false, description: "Ingress type: none, cloudflare, caddy, or ingress_nginx" },
  { name: "erpc_hostname", type: "string", condition: "always", default: '""', sensitive: false, description: "Hostname for eRPC proxy endpoint" },
  { name: "ingress_tls_email", type: "string", condition: "always", default: '""', sensitive: false, description: "Email for Let's Encrypt certificate provisioning" },
  { name: "deployment_target", type: "string", condition: "always", default: '"managed"', sensitive: false, description: "Deployment target" },
  { name: "runtime_arch", type: "string", condition: "always", default: '"multi"', sensitive: false, description: "Runtime architecture: amd64, arm64, or multi" },
  { name: "streaming_mode", type: "string", condition: "always", default: '"disabled"', sensitive: false, description: "Event streaming: disabled, kafka, sns-sqs, or cdc" },

  // ── SSH ───────────────────────────────────────────────────────────────
  { name: "ssh_private_key_path", type: "string", condition: "always", default: '""', sensitive: true, description: "Path to SSH private key for provisioner", group: "SSH" },

  // ── Cloud (AWS) ───────────────────────────────────────────────────────
  { name: "networking_enabled", type: "bool", condition: "cloud", sensitive: false, description: "Enable VPC and networking resources", group: "AWS" },
  { name: "aws_region", type: "string", condition: "cloud", sensitive: false, description: "AWS region for all resources" },
  { name: "ssh_public_key", type: "string", condition: "cloud", sensitive: true, description: "SSH public key for EC2 instances" },
  { name: "network_availability_zones", type: "list(string)", condition: "cloud", sensitive: false, description: "Availability zones for subnets" },
  { name: "network_enable_nat_gateway", type: "bool", condition: "cloud", sensitive: false, description: "Enable NAT gateway for private subnets" },
  { name: "network_environment", type: "string", condition: "cloud", default: '"dev"', sensitive: false, description: "Environment tag (dev, staging, production)" },
  { name: "network_vpc_cidr", type: "string", condition: "cloud", default: '"10.42.0.0/16"', sensitive: false, description: "VPC CIDR block" },
  { name: "network_enable_vpc_endpoints", type: "bool", condition: "cloud", default: "false", sensitive: false, description: "Enable VPC endpoints for AWS services" },

  // ── Bare Metal ────────────────────────────────────────────────────────
  { name: "bare_metal_host", type: "string", condition: "bare_metal", sensitive: true, description: "Bare metal host IP or hostname", group: "Bare Metal" },
  { name: "bare_metal_ssh_user", type: "string", condition: "bare_metal", default: '"ubuntu"', sensitive: false, description: "SSH user for bare metal host" },
  { name: "bare_metal_ssh_port", type: "number", condition: "bare_metal", default: "22", sensitive: false, description: "SSH port for bare metal host" },
  { name: "bare_metal_rpc_proxy_mem_limit", type: "string", condition: "bare_metal", default: '"1g"', sensitive: false, description: "Memory limit for eRPC container" },
  { name: "bare_metal_indexer_mem_limit", type: "string", condition: "bare_metal", default: '"2g"', sensitive: false, description: "Memory limit for indexer container" },
  { name: "bare_metal_secrets_encryption", type: "string", condition: "bare_metal", default: '"none"', sensitive: false, description: "Secrets encryption method" },

  // ── Compute: EC2 ──────────────────────────────────────────────────────
  { name: "ec2_instance_type", type: "string", condition: "ec2", sensitive: false, description: "EC2 instance type", group: "Compute" },
  { name: "ec2_rpc_proxy_mem_limit", type: "string", condition: "ec2", default: '"1g"', sensitive: false, description: "Memory limit for eRPC on EC2" },
  { name: "ec2_indexer_mem_limit", type: "string", condition: "ec2", default: '"2g"', sensitive: false, description: "Memory limit for indexer on EC2" },
  { name: "ec2_secret_recovery_window_in_days", type: "number", condition: "ec2", default: "7", sensitive: false, description: "Secrets Manager recovery window (days)" },

  // ── Compute: k3s ──────────────────────────────────────────────────────
  { name: "k3s_instance_type", type: "string", condition: "cloud_k3s", sensitive: false, description: "EC2 instance type for k3s server" },
  { name: "k3s_version", type: "string", condition: "k8s", default: '"v1.30.4+k3s1"', sensitive: false, description: "k3s release version" },
  { name: "k3s_api_allowed_cidrs", type: "list(string)", condition: "cloud_k3s", default: "[]", sensitive: false, description: "CIDRs allowed to reach k3s API (port 6443)" },

  // ── Indexer / RPC ─────────────────────────────────────────────────────
  { name: "rpc_proxy_enabled", type: "bool", condition: "always", sensitive: false, description: "Enable eRPC proxy deployment", group: "Indexer" },
  { name: "indexer_enabled", type: "bool", condition: "always", sensitive: false, description: "Enable rindexer deployment" },
  { name: "indexer_rpc_url", type: "string", condition: "always", default: '""', sensitive: false, description: "RPC endpoint URL for indexer" },
  { name: "indexer_storage_backend", type: "string", condition: "always", sensitive: false, description: "Storage backend: clickhouse or postgres" },
  { name: "rpc_proxy_image", type: "string", condition: "always", default: '"ghcr.io/erpc/erpc:latest"', sensitive: false, description: "Docker image for eRPC" },
  { name: "indexer_image", type: "string", condition: "always", default: '"ghcr.io/joshstevens19/rindexer:latest"', sensitive: false, description: "Docker image for rindexer" },
  { name: "erpc_config_yaml", type: "string", condition: "always", default: '""', sensitive: false, description: "eRPC configuration YAML content" },
  { name: "rindexer_config_yaml", type: "string", condition: "always", default: '""', sensitive: false, description: "rindexer configuration YAML content" },
  { name: "rindexer_abis", type: "map(string)", condition: "always", default: "{}", sensitive: false, description: "ABI files: { filename = content }" },
  { name: "indexer_extra_env", type: "map(string)", condition: "always", default: "{}", sensitive: false, description: "Extra environment variables for indexer" },
  { name: "indexer_extra_secret_env", type: "map(string)", condition: "always", default: "{}", sensitive: true, description: "Extra sensitive environment variables for indexer" },

  // ── Custom Services ────────────────────────────────────────────────────
  // custom_services is handled specially in generateVariablesTf — not in manifest
  // because its HCL type is too complex for the simple VarEntry model.

  // ── Database: ClickHouse ──────────────────────────────────────────────
  { name: "indexer_clickhouse_url", type: "string", condition: "clickhouse", sensitive: true, description: "ClickHouse connection URL", group: "Database" },
  { name: "indexer_clickhouse_user", type: "string", condition: "clickhouse", default: '"default"', sensitive: false, description: "ClickHouse username" },
  { name: "indexer_clickhouse_password", type: "string", condition: "clickhouse", sensitive: true, description: "ClickHouse password" },
  { name: "indexer_clickhouse_db", type: "string", condition: "clickhouse", default: '"rindexer"', sensitive: false, description: "ClickHouse database name" },

  // ── Database: PostgreSQL ──────────────────────────────────────────────
  { name: "postgres_enabled", type: "bool", condition: "postgres", sensitive: false, description: "Enable PostgreSQL" },
  { name: "indexer_postgres_url", type: "string", condition: "byodb_postgres", sensitive: true, description: "PostgreSQL connection URL" },
  { name: "postgres_instance_class", type: "string", condition: "managed_postgres", default: '"db.t4g.micro"', sensitive: false, description: "RDS instance class" },
  { name: "postgres_engine_version", type: "string", condition: "managed_postgres", default: '"16.4"', sensitive: false, description: "PostgreSQL engine version" },
  { name: "postgres_db_name", type: "string", condition: "managed_postgres", default: '"rindexer"', sensitive: false, description: "Database name" },
  { name: "postgres_db_username", type: "string", condition: "managed_postgres", default: '"rindexer"', sensitive: false, description: "Database username" },
  { name: "postgres_backup_retention", type: "number", condition: "managed_postgres", default: "7", sensitive: false, description: "Backup retention (days)" },
  { name: "postgres_manage_master_user_password", type: "bool", condition: "managed_postgres", default: "true", sensitive: false, description: "Let RDS manage the master password" },
  { name: "postgres_master_password", type: "string", condition: "managed_postgres", default: "null", sensitive: true, description: "Master password (only if manage_master_user_password = false)" },
  { name: "postgres_force_ssl", type: "bool", condition: "managed_postgres", default: "false", sensitive: false, description: "Enforce SSL for PostgreSQL connections" },

  // ── Ingress: Cloudflare ───────────────────────────────────────────────
  { name: "ingress_cloudflare_origin_cert", type: "string", condition: "ingress_cloudflare", sensitive: true, description: "Cloudflare origin certificate (PEM)", group: "Ingress" },
  { name: "ingress_cloudflare_origin_key", type: "string", condition: "ingress_cloudflare", sensitive: true, description: "Cloudflare origin private key (PEM)" },
  { name: "ingress_cloudflare_ssl_mode", type: "string", condition: "ingress_cloudflare", default: '"full_strict"', sensitive: false, description: "Cloudflare SSL mode" },

  // ── Ingress: Caddy ────────────────────────────────────────────────────
  { name: "ingress_caddy_image", type: "string", condition: "ingress_caddy", default: '"caddy:2.9.1-alpine"', sensitive: false, description: "Docker image for Caddy" },
  { name: "ingress_caddy_mem_limit", type: "string", condition: "ingress_caddy", default: '"128m"', sensitive: false, description: "Memory limit for Caddy container" },

  // ── Ingress: nginx ────────────────────────────────────────────────────
  { name: "ingress_nginx_chart_version", type: "string", condition: "ingress_nginx", default: '"4.11.3"', sensitive: false, description: "ingress-nginx Helm chart version" },
  { name: "ingress_cert_manager_chart_version", type: "string", condition: "ingress_nginx", default: '"1.16.2"', sensitive: false, description: "cert-manager Helm chart version" },

  // ── Ingress: TLS shared ───────────────────────────────────────────────
  { name: "ingress_request_body_max_size", type: "string", condition: "ingress_tls", default: '"1m"', sensitive: false, description: "Maximum request body size" },
  { name: "ingress_tls_staging", type: "bool", condition: "ingress_tls", default: "false", sensitive: false, description: "Use Let's Encrypt staging (avoid rate limits during testing)" },
  { name: "ingress_hsts_preload", type: "bool", condition: "ingress_tls", default: "false", sensitive: false, description: "Enable HSTS preload" },
  { name: "ingress_class_name", type: "string", condition: "k8s", default: '"nginx"', sensitive: false, description: "Kubernetes ingress class name" },

  // ── Secrets Management ────────────────────────────────────────────────
  { name: "secrets_manager_secret_arn", type: "string", condition: "secrets_provider", default: '""', sensitive: true, description: "AWS Secrets Manager ARN", group: "Secrets" },
  { name: "secrets_manager_kms_key_id", type: "string", condition: "secrets_provider", default: '""', sensitive: false, description: "AWS KMS key ID for encryption" },
  { name: "external_secret_store_name", type: "string", condition: "secrets_external", default: '""', sensitive: false, description: "External Secrets Operator store name" },
  { name: "external_secret_key", type: "string", condition: "secrets_external", default: '""', sensitive: false, description: "External secret key" },
  { name: "eso_chart_version", type: "string", condition: "secrets_provider_or_external", default: '"0.9.13"', sensitive: false, description: "External Secrets Operator Helm chart version" },

  // ── Monitoring ────────────────────────────────────────────────────────
  { name: "monitoring_enabled", type: "bool", condition: "k8s", default: "false", sensitive: false, description: "Enable Grafana + Prometheus monitoring stack", group: "Monitoring" },
  { name: "kube_prometheus_stack_version", type: "string", condition: "monitoring", default: '"72.6.2"', sensitive: false, description: "kube-prometheus-stack Helm chart version" },
  { name: "grafana_ingress_enabled", type: "bool", condition: "monitoring", default: "true", sensitive: false, description: "Expose Grafana via ingress" },
  { name: "grafana_hostname", type: "string", condition: "monitoring", default: '""', sensitive: false, description: "Grafana hostname for ingress" },
  { name: "alertmanager_route_target", type: "string", condition: "monitoring", default: '"slack"', sensitive: false, description: "Alert routing target: slack, sns, pagerduty" },
  { name: "alertmanager_slack_channel", type: "string", condition: "monitoring", default: '"#alerts"', sensitive: false, description: "Slack channel for alerts" },
  { name: "loki_enabled", type: "bool", condition: "monitoring", default: "false", sensitive: false, description: "Enable Loki log aggregation" },
  { name: "loki_chart_version", type: "string", condition: "monitoring", default: '"6.24.0"', sensitive: false, description: "Loki Helm chart version" },
  { name: "clickhouse_metrics_url", type: "string", condition: "monitoring", default: '""', sensitive: false, description: "ClickHouse metrics endpoint for Grafana" },
];

// ---------------------------------------------------------------------------
// Condition resolver
// ---------------------------------------------------------------------------

function matchesCondition(cond: Condition, state: BuilderState): boolean {
  const isBareMetal = state.provider === "bare_metal";
  const isCloud = !isBareMetal;
  const engine = state.computeEngine;
  const isK8s = engine === "k3s" || engine === "eks";
  const isPostgres = state.databaseProfile.includes("postgres") || state.databaseProfile === "managed_rds";
  const isClickhouse = state.databaseProfile.includes("clickhouse");
  const isManagedPostgres = state.databaseProfile === "managed_rds";
  const hasMonitoring = state.monitoring?.enabled ?? false;

  switch (cond) {
    case "always": return true;
    case "cloud": return isCloud;
    case "bare_metal": return isBareMetal;
    case "postgres": return isPostgres;
    case "byodb_postgres": return isPostgres && !isManagedPostgres;
    case "clickhouse": return isClickhouse;
    case "managed_postgres": return isManagedPostgres;
    case "ec2": return engine === "ec2";
    case "cloud_k3s": return isCloud && engine === "k3s";
    case "k8s": return isK8s;
    case "monitoring": return isK8s && hasMonitoring;
    case "ingress_cloudflare": return state.ingressMode === "cloudflare";
    case "ingress_caddy": return state.ingressMode === "caddy";
    case "ingress_nginx": return state.ingressMode === "ingress_nginx";
    case "ingress_tls": return state.ingressMode === "caddy" || state.ingressMode === "ingress_nginx";
    case "secrets_provider": return state.secretsMode === "provider";
    case "secrets_external": return state.secretsMode === "external";
    case "secrets_provider_or_external": return state.secretsMode === "provider" || state.secretsMode === "external";
  }
}

function activeVars(state: BuilderState): VarEntry[] {
  return MANIFEST.filter((v) => matchesCondition(v.condition, state));
}

// ---------------------------------------------------------------------------
// Header comment
// ---------------------------------------------------------------------------

function header(): string {
  return [
    `# Generated by evm-cloud v${BUILDER_MODULE_VERSION} builder`,
    `# https://app.evm-cloud.xyz`,
    `# Regenerate with the builder or use \`evm-cloud init\` to modify`,
    "",
  ].join("\n");
}

// ---------------------------------------------------------------------------
// versions.tf
// ---------------------------------------------------------------------------

export function generateVersionsTf(state: BuilderState): string {
  const lines: string[] = [header()];

  lines.push("terraform {");
  lines.push('  required_version = ">= 1.5"');
  lines.push("");
  lines.push("  required_providers {");

  if (state.provider === "aws") {
    lines.push("    aws = {");
    lines.push('      source  = "hashicorp/aws"');
    lines.push('      version = "~> 5.0"');
    lines.push("    }");
  }

  lines.push("  }");

  // Backend block (empty, values in .tfbackend file)
  if (state.stateBackend) {
    lines.push("");
    lines.push(`  backend "${state.stateBackend.backend}" {}`);
  }

  lines.push("}");
  lines.push("");

  // Provider block
  if (state.provider === "aws") {
    lines.push("provider \"aws\" {");
    lines.push("  region = var.aws_region");
    lines.push("");
    lines.push("  default_tags {");
    lines.push("    tags = {");
    lines.push('      ManagedBy = "evm-cloud"');
    lines.push("      Project   = var.project_name");
    lines.push("    }");
    lines.push("  }");
    lines.push("}");
    lines.push("");
  }

  return lines.join("\n");
}

// ---------------------------------------------------------------------------
// main.tf
// ---------------------------------------------------------------------------

export function generateMainTf(state: BuilderState): string {
  const lines: string[] = [header()];
  const vars = activeVars(state);

  const moduleSource = `github.com/exomonk/evm-cloud?ref=v${BUILDER_MODULE_VERSION}`;

  lines.push('module "evm_cloud" {');
  lines.push(`  source = "${moduleSource}"`);
  lines.push("");

  // Render variable assignments, grouped by section
  let currentGroup = "";
  for (const v of vars) {
    if (v.group && v.group !== currentGroup) {
      if (currentGroup) lines.push("");
      lines.push(`  # --- ${v.group} ---`);
      currentGroup = v.group;
    }

    // Pad name for alignment (longest name ~45 chars)
    const padded = v.name.padEnd(40);
    lines.push(`  ${padded} = var.${v.name}`);
  }

  // custom_services (complex type, handled separately)
  if (state.customServices.length > 0) {
    lines.push("");
    lines.push("  # --- Custom Services ---");
    lines.push("  custom_services".padEnd(42) + "= var.custom_services");
  }

  lines.push("}");
  lines.push("");

  return lines.join("\n");
}

// ---------------------------------------------------------------------------
// variables.tf
// ---------------------------------------------------------------------------

export function generateVariablesTf(state: BuilderState): string {
  const lines: string[] = [header()];
  const vars = activeVars(state);

  let currentGroup = "";
  for (const v of vars) {
    // Section header
    if (v.group && v.group !== currentGroup) {
      if (currentGroup) lines.push("");
      lines.push("# " + "=".repeat(70));
      lines.push(`# ${v.group}`);
      lines.push("# " + "=".repeat(70));
      lines.push("");
      currentGroup = v.group;
    }

    lines.push(`variable "${v.name}" {`);
    lines.push(`  description = "${v.description}"`);

    // Type + default + sensitive rendering with alignment
    if (v.default != null && v.sensitive) {
      lines.push(`  type      = ${v.type}`);
      lines.push(`  default   = ${v.default}`);
      lines.push(`  sensitive = true`);
    } else if (v.default != null) {
      lines.push(`  type    = ${v.type}`);
      lines.push(`  default = ${v.default}`);
    } else if (v.sensitive) {
      lines.push(`  type      = ${v.type}`);
      lines.push(`  sensitive = true`);
    } else {
      lines.push(`  type = ${v.type}`);
    }

    lines.push("}");
    lines.push("");
  }

  // custom_services (complex type, appended after manifest vars)
  if (state.customServices.length > 0) {
    lines.push("# " + "=".repeat(70));
    lines.push("# Custom Services");
    lines.push("# " + "=".repeat(70));
    lines.push("");
    lines.push('variable "custom_services" {');
    lines.push('  description = "User-defined containerized services deployed alongside the indexer stack."');
    lines.push("  type = list(object({");
    lines.push("    name             = string");
    lines.push("    image            = string");
    lines.push("    port             = number");
    lines.push('    health_path      = optional(string, "/health")');
    lines.push("    replicas         = optional(number, 1)");
    lines.push('    cpu_request      = optional(string, "250m")');
    lines.push('    memory_request   = optional(string, "256Mi")');
    lines.push('    cpu_limit        = optional(string, "500m")');
    lines.push('    memory_limit     = optional(string, "512Mi")');
    lines.push("    env              = optional(map(string), {})");
    lines.push("    secret_env       = optional(map(string), {})");
    lines.push("    ingress_hostname = optional(string)");
    lines.push('    ingress_path     = optional(string, "/")');
    lines.push("    node_role        = optional(string)");
    lines.push("  }))");
    lines.push("  default = []");
    lines.push("}");
    lines.push("");
  }

  return lines.join("\n");
}

// ---------------------------------------------------------------------------
// outputs.tf
// ---------------------------------------------------------------------------

export function generateOutputsTf(): string {
  const lines: string[] = [header()];

  lines.push('output "workload_handoff" {');
  lines.push("  description = \"Deployment handoff data for external deployers\"");
  lines.push("  value       = module.evm_cloud.workload_handoff");
  lines.push("  sensitive   = true");
  lines.push("}");
  lines.push("");

  return lines.join("\n");
}

// ---------------------------------------------------------------------------
// terraform.auto.tfvars
// ---------------------------------------------------------------------------

function inferStorageBackend(profile: string): string {
  if (profile.includes("clickhouse")) return "clickhouse";
  return "postgres";
}

function inferDatabaseMode(profile: string): string {
  if (profile.startsWith("managed")) return "managed";
  return "self_hosted";
}

export function generateTfvarsJson(state: BuilderState): string {
  const isBareMetal = state.provider === "bare_metal";
  const isPostgres = state.databaseProfile.includes("postgres") || state.databaseProfile === "managed_rds";
  const isK8s = state.computeEngine === "k3s" || state.computeEngine === "eks";
  const storageBackend = inferStorageBackend(state.databaseProfile);

  const lines: string[] = [header()];

  // ── Core ──────────────────────────────────────────────────────────────
  section(lines, "Core");
  tfvar(lines, "project_name", state.projectName);
  tfvar(lines, "infrastructure_provider", state.provider);
  tfvar(lines, "compute_engine", state.computeEngine);
  tfvar(lines, "workload_mode", state.workloadMode);
  tfvar(lines, "database_mode", inferDatabaseMode(state.databaseProfile));
  tfvar(lines, "secrets_mode", state.secretsMode);
  tfvar(lines, "deployment_target", "managed");
  tfvar(lines, "runtime_arch", "multi");

  // ── Networking / AWS ──────────────────────────────────────────────────
  if (!isBareMetal) {
    section(lines, "AWS Networking");
    tfvar(lines, "networking_enabled", true);
    tfvar(lines, "aws_region", state.region);
    tfvar(lines, "network_availability_zones", [`${state.region}a`, `${state.region}b`]);
    tfvar(lines, "network_enable_nat_gateway", false);
    tfvar(lines, "network_environment", state.networking?.environment ?? "dev");
    tfvar(lines, "network_vpc_cidr", state.networking?.vpcCidr ?? "10.42.0.0/16");
    tfvar(lines, "network_enable_vpc_endpoints", state.networking?.enableVpcEndpoints ?? false);
  }

  // ── Bare Metal ────────────────────────────────────────────────────────
  if (isBareMetal) {
    section(lines, "Bare Metal");
    tfvar(lines, "bare_metal_ssh_user", "ubuntu");
    tfvar(lines, "bare_metal_ssh_port", 22);
  }

  // ── Compute ───────────────────────────────────────────────────────────
  if (state.computeEngine === "ec2" || (state.computeEngine === "k3s" && !isBareMetal)) {
    section(lines, "Compute");
    if (state.computeEngine === "ec2") {
      tfvar(lines, "ec2_instance_type", state.instanceType);
    }
    if (state.computeEngine === "k3s" && !isBareMetal) {
      tfvar(lines, "k3s_instance_type", state.instanceType);
    }
  }

  // ── Database ──────────────────────────────────────────────────────────
  section(lines, "Database");
  tfvar(lines, "indexer_storage_backend", storageBackend);
  if (isPostgres) {
    tfvar(lines, "postgres_enabled", true);
    if (state.databaseName && state.databaseName !== "rindexer") {
      tfvar(lines, "postgres_db_name", state.databaseName);
    }
  } else {
    tfvar(lines, "indexer_clickhouse_user", "default");
    tfvar(lines, "indexer_clickhouse_db", state.databaseName || "rindexer");
  }

  // ── Indexer / RPC Proxy ───────────────────────────────────────────────
  section(lines, "Indexer / RPC Proxy");
  tfvar(lines, "rpc_proxy_enabled", true);
  tfvar(lines, "indexer_enabled", true);
  tfvar(lines, "indexer_rpc_url", "");
  tfvar(lines, "rpc_proxy_image", "ghcr.io/erpc/erpc:latest");
  tfvar(lines, "indexer_image", "ghcr.io/joshstevens19/rindexer:latest");
  lines.push("");
  lines.push("# Config content — populated by `evm-cloud init` from config/ files");
  tfvar(lines, "erpc_config_yaml", "");
  tfvar(lines, "rindexer_config_yaml", "");
  tfvar(lines, "rindexer_abis", {});
  if (Object.keys(state.extraEnv).length > 0) {
    tfvar(lines, "indexer_extra_env", state.extraEnv);
  } else {
    tfvar(lines, "indexer_extra_env", {});
  }

  // ── Ingress ───────────────────────────────────────────────────────────
  section(lines, "Ingress");
  tfvar(lines, "ingress_mode", state.ingressMode);
  tfvar(lines, "erpc_hostname", state.domain || "");
  tfvar(lines, "ingress_tls_email", state.tlsEmail || "");

  // ── Streaming ─────────────────────────────────────────────────────────
  section(lines, "Streaming");
  tfvar(lines, "streaming_mode", state.streaming?.mode ?? "disabled");

  // ── Monitoring ────────────────────────────────────────────────────────
  if (isK8s && state.monitoring?.enabled) {
    section(lines, "Monitoring");
    tfvar(lines, "monitoring_enabled", true);
    if (state.monitoring.grafanaHostname) {
      tfvar(lines, "grafana_hostname", state.monitoring.grafanaHostname);
    }
    if (state.monitoring.lokiEnabled) {
      tfvar(lines, "loki_enabled", true);
    }
  }

  // Custom services
  if (state.customServices.length > 0) {
    section(lines, "Custom Services");
    lines.push("custom_services = [");
    for (const svc of state.customServices) {
      lines.push("  {");
      lines.push(`    name           = "${svc.name}"`);
      lines.push(`    image          = "${svc.image}"`);
      lines.push(`    port           = ${svc.port}`);
      if (svc.healthPath !== "/health") lines.push(`    health_path    = "${svc.healthPath}"`);
      if (svc.replicas !== 1) lines.push(`    replicas       = ${svc.replicas}`);
      if (svc.cpuRequest !== "250m") lines.push(`    cpu_request    = "${svc.cpuRequest}"`);
      if (svc.memoryRequest !== "256Mi") lines.push(`    memory_request = "${svc.memoryRequest}"`);
      if (svc.cpuLimit !== "500m") lines.push(`    cpu_limit      = "${svc.cpuLimit}"`);
      if (svc.memoryLimit !== "512Mi") lines.push(`    memory_limit   = "${svc.memoryLimit}"`);
      if (Object.keys(svc.env).length > 0) {
        lines.push(`    env = ${hclValue(svc.env)}`);
      }
      if (svc.ingressHostname) lines.push(`    ingress_hostname = "${svc.ingressHostname}"`);
      if (svc.nodeRole) lines.push(`    node_role      = "${svc.nodeRole}"`);
      lines.push("  },");
    }
    lines.push("]");
  }

  return lines.join("\n");
}

function section(lines: string[], name: string) {
  lines.push("");
  lines.push("# " + "─".repeat(70));
  lines.push(`# ${name}`);
  lines.push("# " + "─".repeat(70));
  lines.push("");
}

function tfvar(lines: string[], key: string, value: unknown) {
  lines.push(`${key.padEnd(35)} = ${hclValue(value)}`);
}

function hclValue(value: unknown): string {
  if (typeof value === "string") return `"${value}"`;
  if (typeof value === "boolean") return value ? "true" : "false";
  if (typeof value === "number") return String(value);
  if (Array.isArray(value)) {
    if (value.length === 0) return "[]";
    return `[${value.map((v) => hclValue(v)).join(", ")}]`;
  }
  if (typeof value === "object" && value !== null) {
    const entries = Object.entries(value as Record<string, unknown>);
    if (entries.length === 0) return "{}";
    const inner = entries.map(([k, v]) => `  ${JSON.stringify(k)} = ${hclValue(v)}`).join("\n");
    return `{\n${inner}\n}`;
  }
  return '""';
}

// ---------------------------------------------------------------------------
// secrets.auto.tfvars.example
// ---------------------------------------------------------------------------

export function generateSecretsExample(state: BuilderState): string {
  const vars = activeVars(state);
  // Only include sensitive vars that don't already have a usable default (skip maps/lists with {} or [])
  const sensitiveVars = vars.filter((v) => v.sensitive && v.default !== "{}" && v.default !== "[]");

  if (sensitiveVars.length === 0) return "";

  const example: Record<string, string> = {};
  for (const v of sensitiveVars) {
    example[v.name] = "REPLACE_ME";
  }

  const lines: string[] = [];
  lines.push("# Copy this file to secrets.auto.tfvars and fill in real values.");
  lines.push("# NEVER commit secrets.auto.tfvars to version control.");
  lines.push("");
  for (const [key, val] of Object.entries(example)) {
    lines.push(`${key} = "${val}"`);
  }

  return lines.join("\n");
}

// ---------------------------------------------------------------------------
// backend.tfbackend
// ---------------------------------------------------------------------------

export function generateTfBackend(state: BuilderState): string | null {
  if (!state.stateBackend) return null;

  if (state.stateBackend.backend === "s3") {
    const key = state.stateBackend.key ?? `${state.projectName}/terraform.tfstate`;
    return [
      `bucket         = "${state.stateBackend.bucket}"`,
      `dynamodb_table = "${state.stateBackend.dynamodbTable}"`,
      `region         = "${state.stateBackend.region}"`,
      `key            = "${key}"`,
      `encrypt        = ${state.stateBackend.encrypt}`,
    ].join("\n");
  }

  // GCS
  const prefix = state.stateBackend.prefix ?? state.projectName;
  return [
    `bucket = "${state.stateBackend.bucket}"`,
    `prefix = "${prefix}"`,
  ].join("\n");
}

// ---------------------------------------------------------------------------
// .gitignore
// ---------------------------------------------------------------------------

export function generateGitignore(): string {
  return `# Terraform
.terraform/
*.tfstate
*.tfstate.backup
.terraform.lock.hcl
tfplan

# Secrets — NEVER commit
secrets.auto.tfvars
secrets.auto.tfvars.json
*.pem
*.key

# OS
.DS_Store
`;
}

// ---------------------------------------------------------------------------
// Makefile
// ---------------------------------------------------------------------------

export function generateMakefile(): string {
  return `.PHONY: init plan deploy status logs destroy

init:
\tevm-cloud init

plan:
\tevm-cloud deploy --dry-run

deploy:
\tevm-cloud deploy

status:
\tevm-cloud status

logs:
\tevm-cloud logs

destroy:
\tevm-cloud destroy --yes
`;
}

// ---------------------------------------------------------------------------
// .evm-cloud.json (generator metadata)
// ---------------------------------------------------------------------------

export function generateMetadata(state: BuilderState): string {
  return JSON.stringify({
    generator_version: BUILDER_VERSION,
    module_version: BUILDER_MODULE_VERSION,
    generated_at: new Date().toISOString(),
    project_name: state.projectName,
    provider: state.provider,
    compute_engine: state.computeEngine,
    builder_url: "https://app.evm-cloud.xyz",
  }, null, 2);
}

// ---------------------------------------------------------------------------
// README.md
// ---------------------------------------------------------------------------

export function generateReadme(state: BuilderState): string {
  const backendType = state.stateBackend?.backend ?? "local";
  const backendFile = state.stateBackend
    ? `${state.projectName}.${state.stateBackend.backend}.tfbackend`
    : null;

  return `# evm-cloud: ${state.projectName}

Generated by [evm-cloud builder](https://app.evm-cloud.xyz) on ${new Date().toISOString().split("T")[0]}
Module version: v${BUILDER_MODULE_VERSION}

## Prerequisites

- [evm-cloud CLI](https://evm-cloud.xyz/docs/install-cli) (latest)
- [Terraform](https://developer.hashicorp.com/terraform/install) >= 1.5${state.provider === "aws" ? "\n- AWS CLI configured (`aws configure`)" : ""}

## Quick Start

1. Configure secrets:
   \`\`\`bash
   cp secrets.auto.tfvars.example secrets.auto.tfvars
   # Edit secrets.auto.tfvars with your actual values
   \`\`\`

2. Initialize:
   \`\`\`bash
   evm-cloud init
   \`\`\`

3. Preview:
   \`\`\`bash
   evm-cloud deploy --dry-run
   \`\`\`

4. Deploy:
   \`\`\`bash
   evm-cloud deploy
   \`\`\`

## Day-2 Operations

\`\`\`bash
evm-cloud status        # Check health of all services
evm-cloud logs          # Tail logs (indexer, eRPC, etc.)
evm-cloud logs indexer  # Tail specific service
\`\`\`

## Configuration

| File | Purpose | Edit? |
|------|---------|-------|
| \`evm-cloud.toml\` | Project configuration | Yes — primary config |
| \`terraform.auto.tfvars\` | Terraform variable values | Auto-generated from TOML |
| \`secrets.auto.tfvars\` | Passwords, keys, tokens | Yes (gitignored) |
| \`main.tf\` | Module source and version | Only to upgrade |
${backendFile ? `| \`${backendFile}\` | Remote state backend config | Once during setup |` : ""}

## State Backend

${backendType === "local"
    ? "Using **local state**. For production, configure remote state (S3 or GCS) via \`evm-cloud init\`."
    : `Using **${backendType.toUpperCase()} remote state**. State is stored in \`${state.stateBackend!.bucket}\`.`
}

## Destroy

\`\`\`bash
evm-cloud destroy --yes
\`\`\`

## Troubleshooting

If deployment fails partway through:
1. Run \`evm-cloud status\` to see what was created
2. Read the error message
3. Fix the cause and re-run \`evm-cloud deploy\`

Docs: https://evm-cloud.xyz/docs
`;
}
