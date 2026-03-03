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
      text: "Architecture",
      items: [
        { text: "System Architecture", link: "/docs/architecture" },
        { text: "Core Concepts", link: "/docs/concepts" },
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
          text: "k3s Multi-Node + ClickHouse",
          link: "/docs/examples/k3s-multi-clickhouse",
        },
        {
          text: "Bare Metal + ClickHouse",
          link: "/docs/examples/bare-metal-clickhouse",
        },
        { text: "External EC2", link: "/docs/examples/external-ec2" },
        { text: "External EKS", link: "/docs/examples/external-eks" },
      ],
    },
    {
      text: "Reference",
      items: [
        { text: "Variable Reference", link: "/docs/variable-reference" },
        { text: "Outputs Reference", link: "/docs/outputs-reference" },
        { text: "Cost Estimates", link: "/docs/cost-estimates" },
      ],
    },
    {
      text: "Guides",
      items: [
        {
          text: "Secrets Management",
          link: "/docs/guides/secrets-management",
        },
        {
          text: "Two-Phase Workflow",
          link: "/docs/guides/two-phase-workflow",
        },
        {
          text: "External Deployers",
          link: "/docs/guides/external-deployers",
        },
        { text: "Config Updates", link: "/docs/guides/config-updates" },
        {
          text: "Production Checklist",
          link: "/docs/guides/production-checklist",
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
