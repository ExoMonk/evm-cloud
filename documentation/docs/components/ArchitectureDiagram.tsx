import React from "react";

// ─── Data ────────────────────────────────────────────────────────
// Edit these to update the diagram.
//
// layout: "grid"     → nodes shown as a wrapped row (default)
//         "pipeline"  → nodes shown left-to-right with → arrows
//         "split"     → two sub-groups side by side
//
// To add a branch from the pipeline, add a `branches` array to a node.

type Node = {
  id: string;
  label: string;
  sub?: string;
  optional?: boolean;
  branches?: { label: string; targets: string[] }[];
};

type Layer = {
  label: string;
  color: string;
  layout?: "grid" | "pipeline" | "split";
  nodes: Node[];
  // For "split" layout — group nodes into left/right
  groups?: { label: string; nodeIds: string[] }[];
  // Annotation shown below the layer
  annotation?: string;
};

const layers: Layer[] = [
  {
    label: "CLI",
    color: "#8b5cf6",
    nodes: [
      {
        id: "cli",
        label: "evm-cloud CLI",
        sub: "init · apply · deploy · destroy · kubectl · local",
      },
    ],
  },
  {
    label: "Config",
    color: "#6366f1",
    nodes: [
      { id: "presets", label: "Chain Presets", sub: "ABIs · RPC endpoints · BYO Node" },
      { id: "sizing", label: "Sizing Profiles" },
      { id: "secrets", label: "Secrets" },
      { id: "yaml", label: "YAML Config", sub: "rindexer.yaml · erpc.yaml" },
    ],
  },
  {
    label: "Platform",
    color: "#0ea5e9",
    layout: "split",
    groups: [
      { label: "Providers", nodeIds: ["aws", "gcp", "metal"] },
      { label: "Compute", nodeIds: ["ec2", "eks", "k3s", "docker"] },
    ],
    nodes: [
      { id: "aws", label: "AWS" },
      { id: "gcp", label: "GCP", optional: true },
      { id: "metal", label: "Bare Metal" },
      { id: "ec2", label: "EC2 + Docker" },
      { id: "eks", label: "EKS" },
      { id: "k3s", label: "k3s" },
      { id: "docker", label: "Docker Compose" },
    ],
    annotation: "Provider + compute engine are selected per deployment",
  },
  {
    label: "Networking",
    color: "#06b6d4",
    nodes: [
      { id: "vpc", label: "VPC" },
      { id: "subnets", label: "Subnets" },
      { id: "sg", label: "Security Groups" },
      { id: "bastion", label: "Bastion Host" },
      { id: "dns", label: "DNS", sub: "External", optional: true },
    ],
    annotation: "Provisioned per provider — isolated network per deployment",
  },
  {
    label: "Data Pipeline",
    color: "#10b981",
    layout: "pipeline",
    nodes: [
      { id: "node", label: "EVM Node", sub: "Reth / Erigon", optional: true },
      { id: "erpc", label: "eRPC Proxy", sub: "Failover · Caching" },
      {
        id: "indexer",
        label: "rindexer",
        sub: "No-code or Rust",
        branches: [
          { label: "events", targets: ["streaming"] },
        ],
      },
      {
        id: "db",
        label: "Database",
        sub: "ClickHouse · PostgreSQL",
      },
    ],
  },
  {
    label: "Streaming",
    color: "#f59e0b",
    annotation: "Fan-out from rindexer indexed events",
    nodes: [
      { id: "kafka", label: "Kafka", optional: true },
      { id: "sns", label: "SNS/SQS", optional: true },
      { id: "cdc", label: "CDC", optional: true },
      { id: "webhooks", label: "Webhooks", optional: true },
    ],
  },
  {
    label: "Ops",
    color: "#ef4444",
    layout: "split",
    groups: [
      { label: "Observability", nodeIds: ["grafana", "prometheus", "alerting"] },
      { label: "Ingress", nodeIds: ["tls", "routing"] },
    ],
    annotation: "Monitors all pipeline services · Routes external traffic",
    nodes: [
      { id: "grafana", label: "Grafana", sub: "Dashboards" },
      { id: "prometheus", label: "Prometheus", sub: "Metrics + Rules" },
      { id: "alerting", label: "Alerting", sub: "Slack / SNS / PagerDuty" },
      { id: "tls", label: "TLS", sub: "Cloudflare / Caddy / ingress-nginx" },
      { id: "routing", label: "Routing", sub: "GraphQL · Admin", optional: true },
    ],
  },
];

// ─── Styles ──────────────────────────────────────────────────────

const s = {
  container: {
    display: "flex",
    flexDirection: "column" as const,
    gap: "0",
    fontFamily: "var(--vocs-fontFamily_default, system-ui, sans-serif)",
    fontSize: "13px",
    lineHeight: 1.4,
  },
  // Layer row
  layer: {
    display: "flex",
    alignItems: "stretch" as const,
    borderRadius: "0",
    overflow: "hidden",
    borderBottom: "1px solid var(--vocs-color_background4, #333)",
  },
  layerFirst: { borderRadius: "10px 10px 0 0" as const },
  layerLast: { borderRadius: "0 0 10px 10px" as const, borderBottom: "none" },
  layerLabel: (color: string) => ({
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    width: "100px",
    minWidth: "100px",
    padding: "12px 6px",
    backgroundColor: color,
    color: "#fff",
    fontWeight: 700,
    fontSize: "10px",
    textTransform: "uppercase" as const,
    letterSpacing: "0.08em",
    textAlign: "center" as const,
  }),
  // Grid layout (default)
  gridContainer: {
    display: "flex",
    flexWrap: "wrap" as const,
    gap: "6px",
    padding: "10px 14px",
    flex: 1,
    backgroundColor: "var(--vocs-color_background3, #1a191b)",
    alignItems: "center",
  },
  // Pipeline layout
  pipelineContainer: {
    display: "flex",
    gap: "0",
    padding: "10px 14px",
    flex: 1,
    backgroundColor: "var(--vocs-color_background3, #1a191b)",
    alignItems: "center",
    flexWrap: "wrap" as const,
  },
  pipelineArrow: {
    display: "flex",
    alignItems: "center",
    padding: "0 6px",
    color: "#10b981",
    fontSize: "18px",
    fontWeight: 700,
    userSelect: "none" as const,
  },
  // Split layout
  splitContainer: {
    display: "flex",
    flex: 1,
    backgroundColor: "var(--vocs-color_background3, #1a191b)",
  },
  splitGroup: {
    flex: 1,
    padding: "10px 14px",
    display: "flex",
    flexDirection: "column" as const,
    gap: "6px",
  },
  splitDivider: {
    width: "1px",
    backgroundColor: "var(--vocs-color_background4, #333)",
  },
  splitGroupLabel: {
    fontSize: "10px",
    fontWeight: 600,
    textTransform: "uppercase" as const,
    letterSpacing: "0.06em",
    color: "var(--vocs-color_text3, #838383)",
    marginBottom: "2px",
  },
  splitGroupNodes: {
    display: "flex",
    flexWrap: "wrap" as const,
    gap: "6px",
    alignItems: "center",
  },
  // Node card — available
  node: (optional: boolean) => ({
    padding: "5px 10px",
    borderRadius: "5px",
    backgroundColor: optional
      ? "transparent"
      : "var(--vocs-color_background5, #e8e8e8)",
    border: optional
      ? "1px dashed var(--vocs-color_text4, #555)"
      : "1px solid var(--vocs-color_border, #444)",
    opacity: optional ? 0.4 : 1,
  }),
  nodeLabel: {
    fontWeight: 600,
    color: "var(--vocs-color_heading, #fff)",
    fontSize: "12px",
  },
  nodeLabelPlanned: {
    fontWeight: 500,
    color: "var(--vocs-color_text3, #838383)",
    fontSize: "12px",
  },
  nodeSub: {
    color: "var(--vocs-color_text3, #838383)",
    fontSize: "10px",
    marginTop: "1px",
  },
  nodeSubPlanned: {
    color: "var(--vocs-color_text4, #666)",
    fontSize: "10px",
    marginTop: "1px",
  },
  // Branch indicator
  branchTag: {
    display: "inline-flex",
    alignItems: "center",
    gap: "3px",
    padding: "2px 6px",
    borderRadius: "3px",
    backgroundColor: "#f59e0b20",
    border: "1px solid #f59e0b40",
    color: "#f59e0b",
    fontSize: "9px",
    fontWeight: 600,
    marginLeft: "4px",
    textTransform: "uppercase" as const,
    letterSpacing: "0.04em",
  },
  // Annotation
  annotation: {
    fontSize: "10px",
    color: "var(--vocs-color_text4, #999)",
    fontStyle: "italic" as const,
    padding: "4px 14px 4px 114px",
  },
  // Legend
  legend: {
    display: "flex",
    gap: "16px",
    justifyContent: "flex-end",
    padding: "10px 0 0",
    fontSize: "11px",
    color: "var(--vocs-color_text3, #838383)",
  },
  legendItem: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
  },
  legendBox: (dashed: boolean) => ({
    width: "20px",
    height: "12px",
    borderRadius: "3px",
    border: dashed
      ? "1px dashed var(--vocs-color_text4, #555)"
      : "1px solid var(--vocs-color_border, #444)",
    backgroundColor: dashed
      ? "transparent"
      : "var(--vocs-color_background5, #e8e8e8)",
    opacity: dashed ? 0.4 : 1,
  }),
  legendArrow: {
    color: "#10b981",
    fontWeight: 700,
    fontSize: "14px",
  },
};

// ─── Renderers ───────────────────────────────────────────────────

function NodeCard({ node }: { node: Node }) {
  const planned = !!node.optional;
  return (
    <div style={s.node(planned)}>
      <div style={planned ? s.nodeLabelPlanned : s.nodeLabel}>
        {node.label}
        {node.branches?.map((b) => (
          <span key={b.label} style={s.branchTag}>
            ↗ {b.label}
          </span>
        ))}
      </div>
      {node.sub && (
        <div style={planned ? s.nodeSubPlanned : s.nodeSub}>{node.sub}</div>
      )}
    </div>
  );
}

function GridNodes({ nodes }: { nodes: Node[] }) {
  return (
    <div style={s.gridContainer}>
      {nodes.map((n) => (
        <NodeCard key={n.id} node={n} />
      ))}
    </div>
  );
}

function PipelineNodes({ nodes }: { nodes: Node[] }) {
  return (
    <div style={s.pipelineContainer}>
      {nodes.map((n, i) => (
        <React.Fragment key={n.id}>
          <NodeCard node={n} />
          {i < nodes.length - 1 && <div style={s.pipelineArrow}>→</div>}
        </React.Fragment>
      ))}
    </div>
  );
}

function SplitNodes({ layer }: { layer: Layer }) {
  if (!layer.groups) return <GridNodes nodes={layer.nodes} />;
  const nodeMap = Object.fromEntries(layer.nodes.map((n) => [n.id, n]));
  return (
    <div style={s.splitContainer}>
      {layer.groups.map((group, gi) => (
        <React.Fragment key={group.label}>
          {gi > 0 && <div style={s.splitDivider} />}
          <div style={s.splitGroup}>
            <div style={s.splitGroupLabel}>{group.label}</div>
            <div style={s.splitGroupNodes}>
              {group.nodeIds.map((id) => {
                const node = nodeMap[id];
                return node ? <NodeCard key={id} node={node} /> : null;
              })}
            </div>
          </div>
        </React.Fragment>
      ))}
    </div>
  );
}

// ─── Main Component ──────────────────────────────────────────────

export function ArchitectureDiagram() {
  return (
    <div style={s.container}>
      {layers.map((layer, i) => {
        const isFirst = i === 0;
        const isLast = i === layers.length - 1;
        const layerStyle = {
          ...s.layer,
          ...(isFirst ? s.layerFirst : {}),
          ...(isLast ? s.layerLast : {}),
        };

        return (
          <React.Fragment key={layer.label}>
            <div style={layerStyle}>
              <div style={s.layerLabel(layer.color)}>{layer.label}</div>
              {layer.layout === "pipeline" ? (
                <PipelineNodes nodes={layer.nodes} />
              ) : layer.layout === "split" ? (
                <SplitNodes layer={layer} />
              ) : (
                <GridNodes nodes={layer.nodes} />
              )}
            </div>
            {layer.annotation && (
              <div style={s.annotation}>{layer.annotation}</div>
            )}
          </React.Fragment>
        );
      })}

      <div style={s.legend}>
        <div style={s.legendItem}>
          <div style={s.legendBox(false)} />
          <span>Available</span>
        </div>
        <div style={s.legendItem}>
          <div style={s.legendBox(true)} />
          <span>Planned</span>
        </div>
        <div style={s.legendItem}>
          <span style={s.legendArrow}>→</span>
          <span>Data flow</span>
        </div>
        <div style={s.legendItem}>
          <span style={{ ...s.branchTag, margin: 0 }}>↗</span>
          <span>Branch output</span>
        </div>
      </div>
    </div>
  );
}
