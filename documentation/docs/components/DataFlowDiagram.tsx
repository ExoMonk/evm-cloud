import React from "react";

// ─── Data ────────────────────────────────────────────────────────
// Each step in the pipeline. Edit these to update the diagram.

type FlowNode = {
  id: string;
  label: string;
  color: string;
  items: string[];
  protocol?: string; // label on the arrow coming INTO this node
  optional?: boolean;
};

// Main pipeline: top to bottom
const pipeline: FlowNode[] = [
  {
    id: "upstreams",
    label: "Upstream RPCs",
    color: "#8b5cf6",
    items: ["Infura", "Alchemy", "Llamarpc", "Public endpoints", "BYO Node"],
  },
  {
    id: "erpc",
    label: "eRPC Proxy",
    color: "#0ea5e9",
    items: [
      "Upstream health checks",
      "Automatic failover",
      "Response caching",
      "Rate limit management",
      "Hedged requests",
    ],
    protocol: "JSON-RPC",
  },
  {
    id: "indexer",
    label: "rindexer",
    color: "#10b981",
    items: [
      "Block polling",
      "Event log filtering",
      "ABI decoding",
      "Reorg handling",
      "Batch writes",
    ],
    protocol: "Polling + WebSockets",
  },
];

// Fork: rindexer outputs to both Database and Streaming
const fork: { left: FlowNode; right: FlowNode } = {
  left: {
    id: "db",
    label: "Database",
    color: "#f59e0b",
    items: ["PostgreSQL (RDS managed)", "ClickHouse (BYODB)", "Events, transactions, logs"],
    protocol: "SQL inserts",
  },
  right: {
    id: "streaming",
    label: "Streaming",
    color: "#f97316",
    items: ["Kafka", "SNS/SQS", "Webhooks", "CDC"],
    protocol: "Events",
    optional: true,
  },
};

// ─── Styles ──────────────────────────────────────────────────────

const s = {
  container: {
    display: "flex",
    flexDirection: "column" as const,
    gap: "0",
    fontFamily: "var(--vocs-fontFamily_default, system-ui, sans-serif)",
    fontSize: "13px",
    lineHeight: 1.5,
  },
  node: (color: string) => ({
    display: "flex",
    borderRadius: "8px",
    overflow: "hidden",
    border: `1px solid ${color}40`,
  }),
  nodeLabel: (color: string) => ({
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    width: "120px",
    minWidth: "120px",
    padding: "12px 8px",
    backgroundColor: color,
    color: "#fff",
    fontWeight: 700,
    fontSize: "12px",
    textAlign: "center" as const,
  }),
  nodeContent: {
    flex: 1,
    padding: "10px 16px",
    backgroundColor: "var(--vocs-color_background3, #1a191b)",
    display: "flex",
    flexWrap: "wrap" as const,
    gap: "6px 16px",
    alignItems: "center",
  },
  item: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
    color: "var(--vocs-color_text, #ccc)",
    fontSize: "12px",
  },
  dot: (color: string) => ({
    width: "5px",
    height: "5px",
    borderRadius: "50%",
    backgroundColor: `${color}99`,
    flexShrink: 0,
  }),
  // Arrow between nodes
  arrow: {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    gap: "8px",
    padding: "4px 0",
  },
  arrowLine: {
    display: "flex",
    flexDirection: "column" as const,
    alignItems: "center",
    color: "var(--vocs-color_text4, #666)",
  },
  arrowChar: {
    fontSize: "18px",
    lineHeight: 1,
  },
  arrowProtocol: {
    fontSize: "10px",
    fontWeight: 600,
    color: "var(--vocs-color_text3, #999)",
    textTransform: "uppercase" as const,
    letterSpacing: "0.05em",
    padding: "1px 8px",
    borderRadius: "3px",
    backgroundColor: "var(--vocs-color_background4, #2a2a2a)",
  },
};

// ─── Component ───────────────────────────────────────────────────

function Arrow({ protocol }: { protocol?: string }) {
  return (
    <div style={s.arrow}>
      <div style={s.arrowLine}>
        <span style={s.arrowChar}>↓</span>
      </div>
      {protocol && <span style={s.arrowProtocol}>{protocol}</span>}
    </div>
  );
}

function FlowNodeCard({ node }: { node: FlowNode }) {
  return (
    <div style={{
      ...s.node(node.color),
      opacity: node.optional ? 0.55 : 1,
      borderStyle: node.optional ? "dashed" : "solid",
      flex: 1,
    }}>
      <div style={s.nodeLabel(node.color)}>{node.label}</div>
      <div style={s.nodeContent}>
        {node.items.map((item) => (
          <div key={item} style={s.item}>
            <span style={s.dot(node.color)} />
            {item}
          </div>
        ))}
      </div>
    </div>
  );
}

export function DataFlowDiagram() {
  return (
    <div style={s.container}>
      {/* Main pipeline */}
      {pipeline.map((node, i) => (
        <React.Fragment key={node.id}>
          <FlowNodeCard node={node} />
          {i < pipeline.length - 1 && (
            <Arrow protocol={pipeline[i + 1].protocol} />
          )}
        </React.Fragment>
      ))}

      {/* Fork arrows */}
      <div style={{
        display: "flex",
        gap: "16px",
        padding: "4px 0",
      }}>
        <div style={{ flex: 1, display: "flex", justifyContent: "center", alignItems: "center", gap: "8px" }}>
          <span style={{ fontSize: "18px", color: "var(--vocs-color_text4, #666)" }}>↓</span>
          {fork.left.protocol && <span style={s.arrowProtocol}>{fork.left.protocol}</span>}
        </div>
        <div style={{ flex: 1, display: "flex", justifyContent: "center", alignItems: "center", gap: "8px" }}>
          <span style={{ fontSize: "18px", color: "var(--vocs-color_text4, #666)" }}>↓</span>
          {fork.right.protocol && <span style={s.arrowProtocol}>{fork.right.protocol}</span>}
        </div>
      </div>

      {/* Fork nodes side by side */}
      <div style={{ display: "flex", gap: "16px" }}>
        <FlowNodeCard node={fork.left} />
        <FlowNodeCard node={fork.right} />
      </div>
    </div>
  );
}
