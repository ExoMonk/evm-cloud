import { defineConfig } from "vocs";

export default defineConfig({
  title: "🏰 EVM Cloud",
  iconUrl: "/evm-cloud-favicon.png",
  description:
    "Deploy, manage, and scale EVM blockchain data infrastructure — Nodes, RPC proxies, indexers, databases, and networking — on any cloud or bare metal",
  theme: {
    variables: {
      content: {
        width: "calc(90ch + (var(--vocs-content_horizontalPadding) * 2))",
      },
    },
  },

  topNav: [
    { text: "Docs", link: "/docs/getting-started", match: "/docs" },
    {
      text: "Examples",
      link: "/docs/examples",
      match: "/docs/examples",
    },
    {
      text: "Builder",
      link: "https://app.evm-cloud.xyz",
    },
  ],
  socials: [
    {
      icon: "github",
      link: "https://github.com/ExoMonk/evm-cloud",
    },
  ],
  sidebar: [
    {
      text: "Overview",
      link: "/docs",
    },
    {
      text: "Getting Started",
      link: "/docs/getting-started",
    },
    {
      text: "Use Cases",
      link: "/docs/use-cases",
    },
    {
      text: "Install CLI",
      link: "/docs/install",
    },
    {
      text: "Architecture",
      items: [
        { text: "System Architecture", link: "/docs/architecture" },
        { text: "Core Concepts", link: "/docs/concepts" },
      ],
    },
    {
      text: "Reference",
      items: [
        { text: "CLI Reference", link: "/docs/cli-reference" },
        { text: "Variable Reference", link: "/docs/variable-reference" },
        { text: "Outputs Reference", link: "/docs/outputs-reference" },
        { text: "Cost Estimates", link: "/docs/cost-estimates" },
      ],
    },
    {
      text: "Guides",
      items: [
        { text: "Protocol Templates", link: "/docs/guides/templates" },
        { text: "Rindexer & eRPC Config", link: "/docs/guides/rindexer-config" },
        { text: "Local Dev Stack", link: "/docs/guides/local-dev" },
        {
          text: "TLS & Ingress",
          link: "/docs/guides/tls-ingress",
        },
        {
          text: "Secrets Management",
          link: "/docs/guides/secrets-management",
        },
        {
          text: "State Management",
          link: "/docs/guides/state-management",
        },
        {
          text: "Remote State & .tfbackend",
          link: "/docs/guides/remote-state",
        },
        {
          text: "Two-Phase Workflow",
          link: "/docs/guides/two-phase-workflow",
        },
        {
          text: "Custom Services",
          link: "/docs/guides/custom-services",
        },
        {
          text: "External Deployers",
          link: "/docs/guides/external-deployers",
        },
        { text: "Observability", link: "/docs/guides/observability" },
        { text: "Config Updates", link: "/docs/guides/config-updates" },
        {
          text: "Production",
          link: "/docs/guides/production-checklist",
        },
      ],
    },
    {
      text: "Examples",
      items: [
        { text: "Choosing an Example", link: "/docs/examples" },
        {
          text: "EC2 + Docker + RDS",
          link: "/docs/examples/ec2-docker-compose-rds",
        },
        {
          text: "EC2 + Docker + ClickHouse",
          link: "/docs/examples/ec2-docker-compose-clickhouse",
        },
        { text: "EKS + ClickHouse", link: "/docs/examples/eks-clickhouse" },
        { text: "k3s + ClickHouse", link: "/docs/examples/k3s-clickhouse" },
        {
          text: "Bare Metal + ClickHouse",
          link: "/docs/examples/bare-metal-clickhouse",
        },
        { text: "External EC2", link: "/docs/examples/external-ec2" },
        { text: "k3s + Cloudflare Ingress", link: "/docs/examples/k3s-cloudflare" },
        { text: "Bare Metal k3s + Postgres", link: "/docs/examples/bare-metal-k3s" },
        {
          text: "Prod: k3s Multi-Node + ClickHouse",
          link: "/docs/examples/k3s-multi-clickhouse",
        },
        {
          text: "DeFi: Bare Metal k3s + Swap API",
          link: "/docs/examples/defi-k3s-swap-api",
        },
      ],
    },
    {
      text: "Troubleshooting",
      link: "/docs/troubleshooting",
    },
    {
      text: "Roadmap",
      link: "/docs/roadmap",
    },
  ],
});
