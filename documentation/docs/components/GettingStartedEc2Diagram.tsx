import React from "react";

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
    title: "Step 1 · Configure",
    color: "#8b5cf6",
    items: [
      { label: "minimal_clickhouse.tfvars", sub: "EC2 + BYO ClickHouse" },
      { label: "ssh_public_key", sub: "EC2 access" },
      { label: "ClickHouse URL + Password", sub: "Stored in secrets.auto.tfvars" },
      { label: "rindexer.yaml + ABIs", sub: "Indexer behavior" },
      { label: "Command: evm-cloud init", sub: "Initialize providers" },
    ],
  },
  {
    title: "Step 2 · Deploy",
    color: "#0ea5e9",
    items: [
      { label: "VPC + Security Groups", sub: "Network and host access" },
      { label: "EC2 t3.micro", sub: "Docker host" },
      { label: "IAM Role + Secrets Manager", sub: "Runtime secret delivery" },
      { label: "Docker Compose Stack", sub: "Rendered on /opt/evm-cloud" },
      { label: "Command: evm-cloud deploy", sub: "Creates infra + starts containers" },
    ],
  },
  {
    title: "Step 3 · Verify",
    color: "#10b981",
    items: [
      { label: "eRPC", sub: "RPC proxy on container network" },
      { label: "rindexer", sub: "Indexes events into ClickHouse" },
      { label: "workload_handoff", sub: "public_ip, ssh_command, paths" },
      { label: "Queryable Data", sub: "Events available in BYO ClickHouse" },
      { label: "Command: docker compose ps", sub: "Confirm services are running" },
    ],
  },
];

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

export function GettingStartedEc2Diagram() {
  return (
    <div style={s.container}>
      {columns.map((col, i) => (
        <React.Fragment key={col.title}>
          {i > 0 && <div style={s.arrow}>→</div>}
          <div
            style={{
              ...s.column,
              ...(i === columns.length - 1 ? s.columnLast : {}),
            }}
          >
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
