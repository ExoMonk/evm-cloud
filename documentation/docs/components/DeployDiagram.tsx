import React from "react";

// ─── Data ────────────────────────────────────────────────────────
// Left: what the user provides. Middle: what Terraform creates.
// Right: what's running after apply.

type Item = {
  label: string;
  sub?: string;
};

type Column = {
  title: string;
  color: string;
  items: Item[];
};

const columns: Column[] = [
  {
    title: "You Provide",
    color: "#8b5cf6",
    items: [
      { label: "rindexer.yaml", sub: "Indexer config" },
      { label: "erpc.yaml", sub: "RPC proxy config" },
      { label: "Contract ABIs", sub: "Event definitions" },
      { label: "terraform.tfvars", sub: "Infra settings" },
      { label: "RPC endpoints", sub: "Or BYO node" },
    ],
  },
  {
    title: "Terraform Creates",
    color: "#0ea5e9",
    items: [
      { label: "VPC + Networking", sub: "Subnets, SGs, bastion" },
      { label: "Compute", sub: "EC2 / EKS / k3s / bare metal" },
      { label: "Database", sub: "PostgreSQL (RDS) or BYODB ClickHouse" },
      { label: "Secrets", sub: "AWS Secrets Manager / K8s secrets" },
      { label: "Config Injection", sub: "YAML + ABIs delivered to containers" },
    ],
  },
  {
    title: "You Get",
    color: "#10b981",
    items: [
      { label: "eRPC Proxy", sub: "Multi-upstream failover + caching" },
      { label: "rindexer", sub: "Indexing EVM events to your DB" },
      { label: "Queryable Data", sub: "Events, txs, logs in SQL" },
      { label: "SSH / kubectl", sub: "Full access to your infra" },
    ],
  },
];

// ─── Styles ──────────────────────────────────────────────────────

const s = {
  container: {
    display: "flex",
    gap: "0",
    fontFamily: "var(--vocs-fontFamily_default, system-ui, sans-serif)",
    fontSize: "13px",
    lineHeight: 1.4,
    borderRadius: "10px",
    overflow: "hidden",
    border: "1px solid var(--vocs-color_background4, #333)",
  },
  column: {
    flex: 1,
    display: "flex",
    flexDirection: "column" as const,
    borderRight: "1px solid var(--vocs-color_background4, #333)",
  },
  columnLast: {
    borderRight: "none",
  },
  columnHeader: (color: string) => ({
    padding: "10px 14px",
    backgroundColor: color,
    color: "#fff",
    fontWeight: 700,
    fontSize: "12px",
    textTransform: "uppercase" as const,
    letterSpacing: "0.06em",
    textAlign: "center" as const,
  }),
  columnBody: {
    flex: 1,
    padding: "10px 12px",
    backgroundColor: "var(--vocs-color_background3, #1a191b)",
    display: "flex",
    flexDirection: "column" as const,
    gap: "6px",
  },
  item: {
    padding: "5px 8px",
    borderRadius: "5px",
    backgroundColor: "var(--vocs-color_background5, #e8e8e8)",
    border: "1px solid var(--vocs-color_border, #444)",
  },
  itemLabel: {
    fontWeight: 600,
    color: "var(--vocs-color_heading, #fff)",
    fontSize: "12px",
  },
  itemSub: {
    color: "var(--vocs-color_text3, #838383)",
    fontSize: "10px",
    marginTop: "1px",
  },
  // Arrow column between sections
  arrow: {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: "var(--vocs-color_background3, #1a191b)",
    padding: "0 2px",
    color: "var(--vocs-color_text4, #666)",
    fontSize: "20px",
    fontWeight: 700,
    userSelect: "none" as const,
  },
};

// ─── Component ───────────────────────────────────────────────────

export function DeployDiagram() {
  return (
    <div style={s.container}>
      {columns.map((col, i) => (
        <React.Fragment key={col.title}>
          {i > 0 && <div style={s.arrow}>→</div>}
          <div style={{
            ...s.column,
            ...(i === columns.length - 1 ? s.columnLast : {}),
          }}>
            <div style={s.columnHeader(col.color)}>{col.title}</div>
            <div style={s.columnBody}>
              {col.items.map((item) => (
                <div key={item.label} style={s.item}>
                  <div style={s.itemLabel}>{item.label}</div>
                  {item.sub && <div style={s.itemSub}>{item.sub}</div>}
                </div>
              ))}
            </div>
          </div>
        </React.Fragment>
      ))}
    </div>
  );
}
