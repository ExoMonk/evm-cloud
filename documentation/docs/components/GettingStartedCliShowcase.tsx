import React, { useEffect, useMemo, useState } from "react";

type Mode = "interactive" | "ci";
type LineKind =
  | "cmd"
  | "headlineBlue"
  | "headlineRed"
  | "default"
  | "warn"
  | "spinner"
  | "blank";

type DemoLine = {
  kind: LineKind;
  text: string;
};

const INTERACTIVE_LINES: DemoLine[] = [
  { kind: "cmd", text: "evm-cloud init" },
  { kind: "headlineBlue", text: "   🏰 ✅ Project initialized" },
  { kind: "blank", text: "" },
  { kind: "cmd", text: "evm-cloud apply --dry-run" },
  { kind: "default", text: "     ✓ Ran terraform plan" },
  { kind: "headlineBlue", text: "   🏖️ ✅ Dry run complete - 9s" },
  { kind: "default", text: "      👉🏻 Logs: .evm-cloud/logs/terraform-plan-<ts>.log" },
  { kind: "default", text: "      👉🏻 Output: .evm-cloud/logs/terraform-output-<ts>.json" },
  { kind: "blank", text: "" },
  { kind: "cmd", text: "evm-cloud apply" },
  { kind: "default", text: "     ✓ Ran terraform plan" },
  { kind: "default", text: "     ✔ Apply these changes? (y/N) · yes" },
  { kind: "spinner", text: "     🔄 Terraforming......" },
  { kind: "default", text: "     ✓ Ran terraform apply" },
  { kind: "headlineBlue", text: "   🏰 ✅ Infrastructure deployed - 3m 12s" },
  { kind: "default", text: "      👉🏻 Logs: .evm-cloud/logs/terraform-apply-<ts>.log" },
  { kind: "default", text: "      👉🏻 Output: .evm-cloud/logs/terraform-output-<ts>.json" },
  { kind: "blank", text: "" },
  { kind: "cmd", text: "evm-cloud destroy --yes" },
  { kind: "warn", text: "     🚧 running destroy in interactive mode" },
  { kind: "default", text: "     ✔ Destroy infrastructure? (y/N) · yes" },
  { kind: "spinner", text: "     🔄 Terraforming......" },
  { kind: "default", text: "     ✓ Ran terraform destroy" },
  { kind: "headlineRed", text: "   🏰 🚀 Destroy complete - 55s" },
];

const CI_LINES: DemoLine[] = [
  { kind: "cmd", text: "evm-cloud init" },
  { kind: "headlineBlue", text: "   🏰 ✅ Project initialized" },
  { kind: "blank", text: "" },
  { kind: "cmd", text: "evm-cloud apply --auto-approve" },
  { kind: "default", text: "     ✓ Ran terraform plan" },
  { kind: "spinner", text: "     🔄 Terraforming......" },
  { kind: "default", text: "     ✓ Ran terraform apply" },
  { kind: "headlineBlue", text: "   🏰 ✅ Infrastructure deployed - 3m 05s" },
  { kind: "default", text: "      👉🏻 Logs: .evm-cloud/logs/terraform-apply-<ts>.log" },
  { kind: "default", text: "      👉🏻 Output: .evm-cloud/logs/terraform-output-<ts>.json" },
  { kind: "blank", text: "" },
  { kind: "cmd", text: "evm-cloud destroy --yes --auto-approve" },
  { kind: "warn", text: "     🚧 running destroy in non-interactive mode" },
  { kind: "spinner", text: "     🔄 Terraforming......" },
  { kind: "default", text: "     ✓ Ran terraform destroy" },
  { kind: "headlineRed", text: "   🏰 🚀 Destroy complete - 48s" },
];

const DEPLOYED = [
  { title: "VPC + Subnets", subtitle: "public networking + security groups" },
  { title: "EC2 + Docker", subtitle: "runtime host for workloads" },
  { title: "eRPC Proxy", subtitle: "RPC failover + caching" },
  { title: "rindexer", subtitle: "event indexing pipeline" },
  { title: "Secrets Manager", subtitle: "credential injection" },
];

const styles = {
  root: {
    margin: "20px 0 28px",
    border: "1px solid #30363d",
    borderRadius: "14px",
    overflow: "hidden",
    background: "#0d1117",
  },
  topBar: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "12px 14px",
    borderBottom: "1px solid #30363d",
    background: "#161b22",
    gap: "10px",
    flexWrap: "wrap" as const,
  },
  title: {
    color: "#e6edf3",
    fontWeight: 600,
    fontSize: "14px",
  },
  chips: {
    display: "flex",
    gap: "8px",
  },
  chip: (active: boolean) => ({
    border: "1px solid #30363d",
    borderRadius: "999px",
    padding: "4px 10px",
    fontSize: "12px",
    cursor: "pointer",
    background: active ? "#2563eb33" : "#0d1117",
    color: active ? "#93c5fd" : "#9ca3af",
  }),
  body: {
    display: "grid",
    gridTemplateColumns: "1.4fr 1fr",
  },
  terminalWrap: {
    borderRight: "1px solid #30363d",
    minHeight: "430px",
  },
  terminalBody: {
    padding: "16px 18px",
    fontFamily:
      "'SF Mono', 'Fira Code', 'Cascadia Code', 'JetBrains Mono', monospace",
    fontSize: "13px",
    lineHeight: 1.6,
    color: "#8b949e",
    minHeight: "430px",
  },
  line: {
    whiteSpace: "pre-wrap" as const,
    minHeight: "1.5em",
  },
  side: {
    padding: "14px",
    background: "linear-gradient(180deg, #0b1220 0%, #0d1117 100%)",
  },
  sideTitle: {
    color: "#93c5fd",
    fontSize: "12px",
    letterSpacing: "0.04em",
    textTransform: "uppercase" as const,
    marginBottom: "10px",
  },
  cardStack: {
    perspective: "900px",
  },
  card: (idx: number) => ({
    border: "1px solid #334155",
    background: "linear-gradient(180deg, #111827 0%, #0f172a 100%)",
    borderRadius: "10px",
    padding: "11px 12px",
    marginBottom: "8px",
    transform: `translateY(${idx * 2}px) rotateX(3deg)`,
    boxShadow: "0 6px 16px rgba(0,0,0,0.22)",
  }),
  cardTitle: {
    color: "#e5e7eb",
    fontSize: "13px",
    fontWeight: 600,
  },
  cardSub: {
    color: "#94a3b8",
    fontSize: "12px",
    marginTop: "2px",
  },
  foot: {
    color: "#6b7280",
    fontSize: "12px",
    marginTop: "10px",
  },
} as const;

function lineColor(kind: LineKind): string {
  switch (kind) {
    case "cmd":
      return "#e6edf3";
    case "headlineBlue":
      return "#60a5fa";
    case "headlineRed":
      return "#f87171";
    case "warn":
      return "#fbbf24";
    case "spinner":
      return "#22d3ee";
    case "default":
      return "#e6edf3";
    default:
      return "#8b949e";
  }
}

export function GettingStartedCliShowcase() {
  const [mode, setMode] = useState<Mode>("interactive");
  const [visibleCount, setVisibleCount] = useState(0);

  const lines = useMemo(
    () => (mode === "interactive" ? INTERACTIVE_LINES : CI_LINES),
    [mode]
  );

  useEffect(() => {
    setVisibleCount(0);
    let cancelled = false;
    let timeout: ReturnType<typeof setTimeout>;

    const tick = (idx: number) => {
      if (cancelled) return;
      if (idx <= lines.length) {
        setVisibleCount(idx);
        timeout = setTimeout(() => tick(idx + 1), idx === 0 ? 350 : 220);
      }
    };

    tick(0);

    return () => {
      cancelled = true;
      clearTimeout(timeout);
    };
  }, [lines]);

  return (
    <div style={styles.root}>
      <div style={styles.topBar}>
        <div style={styles.title}>CLI Walkthrough (simulated)</div>
        <div style={styles.chips}>
          <button
            type="button"
            style={styles.chip(mode === "interactive")}
            onClick={() => setMode("interactive")}
          >
            Interactive
          </button>
          <button
            type="button"
            style={styles.chip(mode === "ci")}
            onClick={() => setMode("ci")}
          >
            CI / Non-interactive
          </button>
        </div>
      </div>

      <div style={styles.body}>
        <div style={styles.terminalWrap}>
          <div style={styles.terminalBody}>
            {lines.slice(0, visibleCount).map((line, idx) => (
              <div key={`${mode}-${idx}`} style={{ ...styles.line, color: lineColor(line.kind) }}>
                {line.kind === "cmd" ? `$ ${line.text}` : line.text || "\u00A0"}
              </div>
            ))}
          </div>
        </div>

        <div style={styles.side}>
          <div style={styles.sideTitle}>What gets deployed</div>
          <div style={styles.cardStack}>
            {DEPLOYED.map((item, idx) => (
              <div key={item.title} style={styles.card(idx)}>
                <div style={styles.cardTitle}>{item.title}</div>
                <div style={styles.cardSub}>{item.subtitle}</div>
              </div>
            ))}
          </div>
          <div style={styles.foot}>
            Visual summary for the `minimal_aws_byo_clickhouse` flow.
          </div>
        </div>
      </div>
    </div>
  );
}
