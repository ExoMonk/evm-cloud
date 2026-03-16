/**
 * Golden file test fixtures — 10 config combos (5 infra profiles × 2 DB profiles).
 *
 * These fixtures represent the canonical BuilderState for each deployment pattern.
 * The golden file test generates output from these and compares against saved snapshots.
 */

import { type BuilderState, initialState } from "../src/lib/configSchema.ts";

function fixture(overrides: Partial<BuilderState>): BuilderState {
  return {
    ...initialState,
    projectName: "golden-test",
    chains: ["ethereum"],
    rpcEndpoints: { ethereum: "https://eth-mainnet.g.alchemy.com/v2/DEMO" },
    ...overrides,
  };
}

export interface TestFixture {
  name: string;
  state: BuilderState;
}

export const FIXTURES: TestFixture[] = [
  // ── AWS Simple (EC2) × ClickHouse ──
  {
    name: "aws-simple-clickhouse",
    state: fixture({
      infraProfile: "aws_simple",
      provider: "aws",
      computeEngine: "ec2",
      workloadMode: "terraform",
      region: "us-east-1",
      instanceType: "t3.small",
      databaseProfile: "byodb_clickhouse",
      databaseName: "rindexer",
      ingressMode: "none",
      secretsMode: "inline",
    }),
  },
  // ── AWS Simple (EC2) × Postgres ──
  {
    name: "aws-simple-postgres",
    state: fixture({
      infraProfile: "aws_simple",
      provider: "aws",
      computeEngine: "ec2",
      workloadMode: "terraform",
      region: "us-east-1",
      instanceType: "t3.small",
      databaseProfile: "managed_rds",
      databaseName: "rindexer",
      ingressMode: "none",
      secretsMode: "provider",
    }),
  },
  // ── AWS Budget K8s (k3s) × ClickHouse ──
  {
    name: "aws-k3s-clickhouse",
    state: fixture({
      infraProfile: "aws_budget_k8s",
      provider: "aws",
      computeEngine: "k3s",
      workloadMode: "external",
      region: "us-east-1",
      instanceType: "t3.small",
      databaseProfile: "byodb_clickhouse",
      databaseName: "rindexer",
      ingressMode: "caddy",
      domain: "rpc.example.com",
      tlsEmail: "admin@example.com",
      secretsMode: "inline",
    }),
  },
  // ── AWS Budget K8s (k3s) × Postgres ──
  {
    name: "aws-k3s-postgres",
    state: fixture({
      infraProfile: "aws_budget_k8s",
      provider: "aws",
      computeEngine: "k3s",
      workloadMode: "external",
      region: "eu-west-1",
      instanceType: "t3.medium",
      databaseProfile: "byodb_postgres",
      databaseName: "indexer_db",
      ingressMode: "ingress_nginx",
      domain: "rpc.example.com",
      tlsEmail: "admin@example.com",
      secretsMode: "inline",
    }),
  },
  // ── AWS Managed K8s (EKS) × ClickHouse ──
  {
    name: "aws-eks-clickhouse",
    state: fixture({
      infraProfile: "aws_managed_k8s",
      provider: "aws",
      computeEngine: "eks",
      workloadMode: "external",
      region: "us-west-2",
      instanceType: "t3.medium",
      databaseProfile: "byodb_clickhouse",
      databaseName: "analytics",
      chains: ["ethereum", "polygon"],
      rpcEndpoints: {
        ethereum: "https://eth-mainnet.g.alchemy.com/v2/DEMO",
        polygon: "https://polygon-mainnet.g.alchemy.com/v2/DEMO",
      },
      ingressMode: "cloudflare",
      domain: "rpc.example.com",
      secretsMode: "provider",
    }),
  },
  // ── AWS Managed K8s (EKS) × Postgres ──
  {
    name: "aws-eks-postgres",
    state: fixture({
      infraProfile: "aws_managed_k8s",
      provider: "aws",
      computeEngine: "eks",
      workloadMode: "external",
      region: "us-east-1",
      instanceType: "t3.large",
      databaseProfile: "managed_rds",
      databaseName: "rindexer",
      ingressMode: "ingress_nginx",
      domain: "rpc.example.com",
      tlsEmail: "admin@example.com",
      secretsMode: "provider",
      monitoring: { enabled: true, grafanaHostname: "grafana.example.com", lokiEnabled: true },
    }),
  },
  // ── VPS Docker × ClickHouse ──
  {
    name: "vps-docker-clickhouse",
    state: fixture({
      infraProfile: "vps_docker",
      provider: "bare_metal",
      computeEngine: "docker_compose",
      workloadMode: "terraform",
      region: "",
      instanceType: "",
      databaseProfile: "byodb_clickhouse",
      databaseName: "rindexer",
      ingressMode: "caddy",
      domain: "rpc.myvps.com",
      tlsEmail: "admin@myvps.com",
      secretsMode: "inline",
    }),
  },
  // ── VPS Docker × Postgres ──
  {
    name: "vps-docker-postgres",
    state: fixture({
      infraProfile: "vps_docker",
      provider: "bare_metal",
      computeEngine: "docker_compose",
      workloadMode: "terraform",
      region: "",
      instanceType: "",
      databaseProfile: "byodb_postgres",
      databaseName: "rindexer",
      ingressMode: "none",
      secretsMode: "inline",
    }),
  },
  // ── VPS K8s (k3s) × ClickHouse ──
  {
    name: "vps-k3s-clickhouse",
    state: fixture({
      infraProfile: "vps_k8s",
      provider: "bare_metal",
      computeEngine: "k3s",
      workloadMode: "external",
      region: "",
      instanceType: "",
      databaseProfile: "byodb_clickhouse",
      databaseName: "rindexer",
      ingressMode: "caddy",
      domain: "rpc.myvps.com",
      tlsEmail: "admin@myvps.com",
      secretsMode: "inline",
    }),
  },
  // ── VPS K8s (k3s) × Postgres ──
  {
    name: "vps-k3s-postgres",
    state: fixture({
      infraProfile: "vps_k8s",
      provider: "bare_metal",
      computeEngine: "k3s",
      workloadMode: "external",
      region: "",
      instanceType: "",
      databaseProfile: "byodb_postgres",
      databaseName: "rindexer",
      ingressMode: "none",
      secretsMode: "inline",
    }),
  },
];
