/**
 * TypeScript mirror of cli/src/config/schema.rs + cli/src/init_answers.rs
 * This is the single source of truth for the builder's state shape.
 *
 * IMPORTANT: When the Rust CLI schema changes, this file must be updated.
 * The schema contract test (M1) catches drift automatically.
 */

// --- Module version pinning (M2) ---
export const BUILDER_VERSION = "0.1.0";
export const BUILDER_MODULE_VERSION = "0.0.1-alpha8";

// --- Enums mirroring schema.rs ---

export type InfraProvider = "aws" | "bare_metal";

export type ComputeEngine = "ec2" | "eks" | "k3s" | "docker_compose";

export type WorkloadMode = "terraform" | "external";

export type DatabaseProfile =
  | "byodb_clickhouse"
  | "byodb_postgres"
  | "managed_rds"
  | "managed_clickhouse";

export type StorageBackend = "clickhouse" | "postgres" | "timescaledb";

export type IngressMode = "none" | "cloudflare" | "caddy" | "ingress_nginx";

export type SecretsMode = "inline" | "provider" | "external";

export type StreamingMode = "disabled" | "kafka" | "sns-sqs" | "cdc";

export type IndexerType = "rindexer" | "custom";

export type TemplateCategory = "token" | "nft" | "dex" | "lending";

export type QueryPattern = "analytics" | "lookups";

// --- Infrastructure Profiles ---

export type InfraProfile =
  | "aws_simple"
  | "aws_budget_k8s"
  | "aws_managed_k8s"
  | "vps_docker"
  | "vps_k8s";

export interface InfraProfileDef {
  id: InfraProfile;
  name: string;
  description: string;
  provider: InfraProvider;
  engine: ComputeEngine;
  defaultWorkloadMode: WorkloadMode;
  costRange: string;
  defaultInstanceType: string | null;
}

export const INFRA_PROFILES: InfraProfileDef[] = [
  {
    id: "aws_simple",
    name: "AWS Simple",
    description: "Single EC2 instance with Docker Compose. Simplest AWS deployment.",
    provider: "aws",
    engine: "ec2",
    defaultWorkloadMode: "terraform",
    costRange: "$15-35/mo",
    defaultInstanceType: "t3.small",
  },
  {
    id: "aws_budget_k8s",
    name: "AWS Budget K8s",
    description: "k3s on EC2 — real Kubernetes without the EKS price tag.",
    provider: "aws",
    engine: "k3s",
    defaultWorkloadMode: "external",
    costRange: "$35-50/mo",
    defaultInstanceType: "t3.small",
  },
  {
    id: "aws_managed_k8s",
    name: "AWS Managed K8s",
    description: "EKS — managed control plane, node groups, autoscaling, RBAC.",
    provider: "aws",
    engine: "eks",
    defaultWorkloadMode: "external",
    costRange: "$110-140/mo",
    defaultInstanceType: "t3.medium",
  },
  {
    id: "vps_docker",
    name: "Any VPS with Docker",
    description: "Docker Compose on any server. No cloud account needed.",
    provider: "bare_metal",
    engine: "docker_compose",
    defaultWorkloadMode: "terraform",
    costRange: "$5-20/mo",
    defaultInstanceType: null,
  },
  {
    id: "vps_k8s",
    name: "Any VPS with K8s",
    description: "k3s on your own server. Real orchestration, zero cloud bill.",
    provider: "bare_metal",
    engine: "k3s",
    defaultWorkloadMode: "external",
    costRange: "VPS cost",
    defaultInstanceType: null,
  },
];

// --- Constraint maps (from schema.rs valid_for_provider / options_for_engine) ---

export const COMPUTE_ENGINES_BY_PROVIDER: Record<InfraProvider, ComputeEngine[]> = {
  aws: ["ec2", "eks", "k3s"],
  bare_metal: ["k3s", "docker_compose"],
};

export const INGRESS_MODES_BY_ENGINE: Record<ComputeEngine, IngressMode[]> = {
  k3s: ["none", "cloudflare", "caddy", "ingress_nginx"],
  eks: ["none", "cloudflare", "caddy", "ingress_nginx"],
  ec2: ["none", "cloudflare", "caddy"],
  docker_compose: ["none", "cloudflare", "caddy"],
};

// --- Available chains (from init_wizard.rs L288-295) ---

export const AVAILABLE_CHAINS = [
  "ethereum",
  "polygon",
  "arbitrum",
  "base",
  "optimism",
  "hyperliquid",
] as const;

export type Chain = (typeof AVAILABLE_CHAINS)[number];

// --- AWS regions ---

export const AWS_REGIONS = [
  "us-east-1",
  "us-east-2",
  "us-west-1",
  "us-west-2",
  "eu-west-1",
  "eu-west-2",
  "eu-central-1",
  "ap-southeast-1",
  "ap-northeast-1",
] as const;

// --- State types ---

export interface S3State {
  backend: "s3";
  bucket: string;
  dynamodbTable: string;
  region: string;
  key?: string;
  encrypt: boolean;
}

export interface GcsState {
  backend: "gcs";
  bucket: string;
  region: string;
  prefix?: string;
}

export interface MonitoringConfig {
  enabled: boolean;
  grafanaHostname?: string;
  alertmanagerSlackChannel?: string;
  lokiEnabled?: boolean;
}

export interface StreamingConfig {
  mode: StreamingMode;
}

export interface NetworkingConfig {
  vpcCidr?: string;
  enableVpcEndpoints?: boolean;
  environment?: string;
}

export interface ContainerConfig {
  rpcProxyImage?: string;
  indexerImage?: string;
}

export interface CustomService {
  name: string;
  image: string;
  port: number;
  healthPath: string;
  replicas: number;
  cpuRequest: string;
  cpuLimit: string;
  memoryRequest: string;
  memoryLimit: string;
  env: Record<string, string>;
  ingressHostname: string;
  nodeRole: string;
}

// --- Builder State ---

export interface BuilderState {
  // Step tracking
  currentStep: number;
  completedSteps: Set<number>;

  // Template (entry point)
  selectedTemplate: string | null;
  templateVariables: Record<string, string>;

  // Infrastructure Profile
  infraProfile: InfraProfile | null;
  provider: InfraProvider;
  computeEngine: ComputeEngine;
  workloadMode: WorkloadMode;
  region: string;
  instanceType: string;

  // Database
  databaseProfile: DatabaseProfile;
  queryPattern: QueryPattern | null;
  databaseName: string;

  // Project + Chains
  projectName: string;
  chains: string[];
  rpcEndpoints: Record<string, string>;

  // Ingress
  ingressMode: IngressMode;
  domain: string;
  tlsEmail: string;

  // Advanced (Review step accordions)
  secretsMode: SecretsMode;
  monitoring: MonitoringConfig | null;
  streaming: StreamingConfig | null;
  networking: NetworkingConfig | null;
  stateBackend: S3State | GcsState | null;
  containers: ContainerConfig | null;
  indexerType: IndexerType;
  customIndexerImage: string;
  extraEnv: Record<string, string>;
  customServices: CustomService[];
}

// --- Initial State ---

export const initialState: BuilderState = {
  currentStep: 0,
  completedSteps: new Set(),

  selectedTemplate: null,
  templateVariables: {},

  infraProfile: null,
  provider: "aws",
  computeEngine: "ec2",
  workloadMode: "terraform",
  region: "us-east-1",
  instanceType: "t3.small",

  databaseProfile: "byodb_clickhouse",
  queryPattern: null,
  databaseName: "rindexer",

  projectName: "evm-cloud-demo",
  chains: [],
  rpcEndpoints: {},

  ingressMode: "none",
  domain: "",
  tlsEmail: "",

  secretsMode: "inline",
  monitoring: null,
  streaming: null,
  networking: null,
  stateBackend: null,
  containers: null,
  indexerType: "rindexer",
  customIndexerImage: "",
  extraEnv: {},
  customServices: [],
};

// --- Actions ---

export type BuilderAction =
  | { type: "SET_STEP"; step: number }
  | { type: "COMPLETE_STEP"; step: number }
  | { type: "SELECT_TEMPLATE"; template: string | null; chains?: string[]; variables?: Record<string, string> }
  | { type: "SET_INFRA_PROFILE"; profile: InfraProfile }
  | { type: "SET_DATABASE_PROFILE"; profile: DatabaseProfile }
  | { type: "SET_DATABASE_NAME"; name: string }
  | { type: "SET_QUERY_PATTERN"; pattern: QueryPattern }
  | { type: "SET_PROJECT_NAME"; name: string }
  | { type: "SET_CHAINS"; chains: string[] }
  | { type: "SET_RPC_ENDPOINT"; chain: string; url: string }
  | { type: "SET_REGION"; region: string }
  | { type: "SET_INSTANCE_TYPE"; instanceType: string }
  | { type: "SET_INGRESS_MODE"; mode: IngressMode }
  | { type: "SET_DOMAIN"; domain: string }
  | { type: "SET_TLS_EMAIL"; email: string }
  | { type: "SET_SECRETS_MODE"; mode: SecretsMode }
  | { type: "SET_MONITORING"; config: MonitoringConfig | null }
  | { type: "SET_STREAMING"; config: StreamingConfig | null }
  | { type: "SET_NETWORKING"; config: NetworkingConfig | null }
  | { type: "SET_STATE_BACKEND"; backend: S3State | GcsState | null }
  | { type: "SET_CONTAINERS"; config: ContainerConfig | null }
  | { type: "SET_INDEXER_TYPE"; indexerType: IndexerType }
  | { type: "SET_CUSTOM_INDEXER_IMAGE"; image: string }
  | { type: "SET_EXTRA_ENV"; env: Record<string, string> }
  | { type: "SET_CUSTOM_SERVICES"; services: CustomService[] }
  | { type: "SET_TEMPLATE_VARIABLE"; key: string; value: string };

// --- Helpers ---

function inferSecretsMode(provider: InfraProvider, engine: ComputeEngine): SecretsMode {
  if (provider === "bare_metal" || engine === "k3s") return "inline";
  return "provider";
}

function inferWorkloadMode(engine: ComputeEngine): WorkloadMode {
  if (engine === "k3s" || engine === "eks") return "external";
  return "terraform";
}

// --- Reducer ---

export function builderReducer(state: BuilderState, action: BuilderAction): BuilderState {
  switch (action.type) {
    case "SET_STEP":
      return { ...state, currentStep: action.step };

    case "COMPLETE_STEP": {
      const completed = new Set(state.completedSteps);
      completed.add(action.step);
      return { ...state, completedSteps: completed };
    }

    case "SELECT_TEMPLATE":
      return {
        ...state,
        selectedTemplate: action.template,
        chains: action.chains ?? state.chains,
        templateVariables: action.variables ?? {},
      };

    case "SET_INFRA_PROFILE": {
      const profile = INFRA_PROFILES.find((p) => p.id === action.profile);
      if (!profile) return state;

      const newEngine = profile.engine;
      const newProvider = profile.provider;
      const validIngress = INGRESS_MODES_BY_ENGINE[newEngine];
      const newIngressMode = validIngress.includes(state.ingressMode)
        ? state.ingressMode
        : "none";

      return {
        ...state,
        infraProfile: action.profile,
        provider: newProvider,
        computeEngine: newEngine,
        workloadMode: inferWorkloadMode(newEngine),
        instanceType: profile.defaultInstanceType ?? "",
        region: newProvider === "bare_metal" ? "" : state.region || "us-east-1",
        secretsMode: inferSecretsMode(newProvider, newEngine),
        ingressMode: newIngressMode,
      };
    }

    case "SET_DATABASE_PROFILE":
      return { ...state, databaseProfile: action.profile };

    case "SET_DATABASE_NAME":
      return { ...state, databaseName: action.name };

    case "SET_QUERY_PATTERN":
      return { ...state, queryPattern: action.pattern };

    case "SET_PROJECT_NAME":
      return { ...state, projectName: action.name };

    case "SET_CHAINS": {
      // Remove orphaned RPC endpoints
      const rpc = { ...state.rpcEndpoints };
      for (const chain of Object.keys(rpc)) {
        if (!action.chains.includes(chain)) {
          delete rpc[chain];
        }
      }
      return { ...state, chains: action.chains, rpcEndpoints: rpc };
    }

    case "SET_RPC_ENDPOINT":
      return {
        ...state,
        rpcEndpoints: { ...state.rpcEndpoints, [action.chain]: action.url },
      };

    case "SET_REGION":
      return { ...state, region: action.region };

    case "SET_INSTANCE_TYPE":
      return { ...state, instanceType: action.instanceType };

    case "SET_INGRESS_MODE":
      return { ...state, ingressMode: action.mode };

    case "SET_DOMAIN":
      return { ...state, domain: action.domain };

    case "SET_TLS_EMAIL":
      return { ...state, tlsEmail: action.email };

    case "SET_SECRETS_MODE":
      return { ...state, secretsMode: action.mode };

    case "SET_MONITORING":
      return { ...state, monitoring: action.config };

    case "SET_STREAMING":
      return { ...state, streaming: action.config };

    case "SET_NETWORKING":
      return { ...state, networking: action.config };

    case "SET_STATE_BACKEND":
      return { ...state, stateBackend: action.backend };

    case "SET_CONTAINERS":
      return { ...state, containers: action.config };

    case "SET_INDEXER_TYPE":
      return { ...state, indexerType: action.indexerType };

    case "SET_CUSTOM_INDEXER_IMAGE":
      return { ...state, customIndexerImage: action.image };

    case "SET_EXTRA_ENV":
      return { ...state, extraEnv: action.env };

    case "SET_CUSTOM_SERVICES":
      return { ...state, customServices: action.services };

    case "SET_TEMPLATE_VARIABLE":
      return {
        ...state,
        templateVariables: { ...state.templateVariables, [action.key]: action.value },
      };

    default:
      return state;
  }
}
