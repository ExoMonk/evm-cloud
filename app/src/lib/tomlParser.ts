/**
 * Minimal TOML parser for evm-cloud.toml files.
 * Only handles the flat sections we generate — not a full TOML spec parser.
 * Good enough to parse our own output and import it back.
 */

export interface ParsedToml {
  [section: string]: Record<string, string | boolean | number | string[]>;
}

export function parseToml(input: string): ParsedToml {
  const result: ParsedToml = { "": {} };
  let currentSection = "";

  for (const rawLine of input.split("\n")) {
    const line = rawLine.trim();

    // Skip comments and empty lines
    if (!line || line.startsWith("#")) continue;

    // Section header: [section] or [section.subsection]
    const sectionMatch = line.match(/^\[([^\]]+)\]$/);
    if (sectionMatch) {
      currentSection = sectionMatch[1];
      if (!result[currentSection]) result[currentSection] = {};
      continue;
    }

    // Key = value
    const kvMatch = line.match(/^(\w+)\s*=\s*(.+)$/);
    if (kvMatch) {
      const [, key, rawValue] = kvMatch;
      const section = currentSection || "";
      if (!result[section]) result[section] = {};
      result[section][key] = parseValue(rawValue.trim());
    }
  }

  return result;
}

function parseValue(raw: string): string | boolean | number | string[] {
  // Boolean
  if (raw === "true") return true;
  if (raw === "false") return false;

  // Number (integer only for our use case)
  if (/^\d+$/.test(raw)) return parseInt(raw, 10);

  // String (quoted)
  if (raw.startsWith('"') && raw.endsWith('"')) {
    return raw.slice(1, -1);
  }

  // Array of strings: ["a", "b"]
  if (raw.startsWith("[") && raw.endsWith("]")) {
    const inner = raw.slice(1, -1).trim();
    if (!inner) return [];
    return inner.split(",").map((s) => {
      const trimmed = s.trim();
      if (trimmed.startsWith('"') && trimmed.endsWith('"')) {
        return trimmed.slice(1, -1);
      }
      return trimmed;
    });
  }

  return raw;
}

/**
 * Map parsed TOML back to a partial BuilderState.
 */
export function tomlToBuilderState(toml: ParsedToml): Record<string, unknown> {
  const state: Record<string, unknown> = {};

  // Project
  const project = toml["project"];
  if (project) {
    if (project.name) state.projectName = project.name;
    if (project.region) state.region = project.region;
  }

  // Compute
  const compute = toml["compute"];
  if (compute) {
    if (compute.engine) state.computeEngine = compute.engine;
    if (compute.instance_type) state.instanceType = compute.instance_type;
  }

  // Database
  const database = toml["database"];
  if (database) {
    if (database.provider) state.provider = database.provider;
    const backend = database.storage_backend as string;
    const mode = database.mode as string;
    if (backend === "clickhouse" && mode === "self_hosted") state.databaseProfile = "byodb_clickhouse";
    else if (backend === "clickhouse" && mode === "managed") state.databaseProfile = "managed_clickhouse";
    else if (backend === "postgres" && mode === "managed") state.databaseProfile = "managed_rds";
    else if (backend === "postgres") state.databaseProfile = "byodb_postgres";
  }

  // Indexer
  const indexer = toml["indexer"];
  if (indexer) {
    if (indexer.chains) state.chains = indexer.chains;
  }

  // RPC endpoints
  const rpc = toml["rpc.endpoints"] ?? toml["rpc"];
  if (rpc) {
    const endpoints: Record<string, string> = {};
    for (const [key, val] of Object.entries(rpc)) {
      if (key !== "endpoints" && typeof val === "string") {
        endpoints[key] = val;
      }
    }
    if (Object.keys(endpoints).length > 0) state.rpcEndpoints = endpoints;
  }

  // Ingress
  const ingress = toml["ingress"];
  if (ingress) {
    if (ingress.mode) state.ingressMode = ingress.mode;
    if (ingress.domain) state.domain = ingress.domain;
    if (ingress.tls_email) state.tlsEmail = ingress.tls_email;
  }

  // Secrets
  const secrets = toml["secrets"];
  if (secrets) {
    if (secrets.mode) state.secretsMode = secrets.mode;
  }

  // Monitoring
  const monitoring = toml["monitoring"];
  if (monitoring) {
    state.monitoring = {
      enabled: monitoring.enabled === true,
      grafanaHostname: monitoring.grafana_hostname as string | undefined,
      lokiEnabled: monitoring.loki_enabled === true,
      alertmanagerSlackChannel: monitoring.alertmanager_slack_channel as string | undefined,
    };
  }

  // Streaming
  const streaming = toml["streaming"];
  if (streaming && streaming.mode && streaming.mode !== "disabled") {
    state.streaming = { mode: streaming.mode };
  }

  // Networking
  const networking = toml["networking"];
  if (networking) {
    state.networking = {
      vpcCidr: networking.vpc_cidr as string | undefined,
      enableVpcEndpoints: networking.enable_vpc_endpoints as boolean | undefined,
      environment: networking.environment as string | undefined,
    };
  }

  // State backend
  const stateConfig = toml["state"];
  if (stateConfig && stateConfig.backend) {
    if (stateConfig.backend === "s3") {
      state.stateBackend = {
        backend: "s3",
        bucket: stateConfig.bucket as string ?? "",
        dynamodbTable: stateConfig.dynamodb_table as string ?? "",
        region: stateConfig.region as string ?? "",
        key: stateConfig.key as string | undefined,
        encrypt: stateConfig.encrypt !== false,
      };
    } else if (stateConfig.backend === "gcs") {
      state.stateBackend = {
        backend: "gcs",
        bucket: stateConfig.bucket as string ?? "",
        region: stateConfig.region as string ?? "",
        prefix: stateConfig.prefix as string | undefined,
      };
    }
  }

  // Infer infra profile from provider + engine
  const provider = (state.provider ?? "aws") as string;
  const engine = (state.computeEngine ?? "ec2") as string;
  if (provider === "aws" && engine === "ec2") state.infraProfile = "aws_simple";
  else if (provider === "aws" && engine === "k3s") state.infraProfile = "aws_budget_k8s";
  else if (provider === "aws" && engine === "eks") state.infraProfile = "aws_managed_k8s";
  else if (provider === "bare_metal" && engine === "docker_compose") state.infraProfile = "vps_docker";
  else if (provider === "bare_metal" && engine === "k3s") state.infraProfile = "vps_k8s";

  // Infer workload mode
  if (engine === "k3s" || engine === "eks") state.workloadMode = "external";
  else state.workloadMode = "terraform";

  return state;
}
