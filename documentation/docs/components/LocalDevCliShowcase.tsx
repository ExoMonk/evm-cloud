import React, { useEffect, useMemo, useState } from "react";

type Mode = "fork" | "fresh";
type LineKind = "cmd" | "headlineBlue" | "headlineRed" | "default" | "warn" | "spinner" | "blank";

type DemoLine = {
  kind: LineKind;
  text: string;
};

const FORK_LINES: DemoLine[] = [
  { kind: "cmd", text: "evm-cloud local up" },
  { kind: "headlineBlue", text: "   🏰 ⚒️ Starting local dev stack" },
  { kind: "default", text: "     ✓ Kind cluster ready" },
  { kind: "spinner", text: "     🔄 Deploying ClickHouse" },
  { kind: "default", text: "     ✓ ClickHouse ready — localhost:8123" },
  { kind: "spinner", text: "     🔄 Deploying Anvil" },
  { kind: "default", text: "     ✓ Anvil ready — localhost:8545" },
  { kind: "spinner", text: "     🔄 Deploying eRPC" },
  { kind: "default", text: "     ✓ eRPC ready — localhost:4000" },
  { kind: "spinner", text: "     🔄 Deploying rindexer" },
  { kind: "default", text: "     ✓ 🦀rindexer ready — localhost:18080" },
  { kind: "default", text: "     ✓ All health checks passed" },
  { kind: "headlineBlue", text: "   🏰 ✅ Local stack ready — 2m 03s" },
  { kind: "default", text: "      👉🏻 Anvil         http://localhost:8545" },
  { kind: "default", text: "      👉🏻 eRPC          http://localhost:4000" },
  { kind: "default", text: "      👉🏻 ClickHouse    http://localhost:8123" },
  { kind: "default", text: "      👉🏻 🦀rindexer    http://localhost:18080" },
  { kind: "default", text: "      👉🏻 Status        evm-cloud local status" },
  { kind: "default", text: "      👉🏻 Tear down     evm-cloud local down" },
];

const FRESH_LINES: DemoLine[] = [
  { kind: "cmd", text: "evm-cloud local up --fresh" },
  { kind: "headlineBlue", text: "   🏰 ⚒️ Starting local dev stack" },
  { kind: "default", text: "     ✓ Kind cluster ready" },
  { kind: "spinner", text: "     🔄 Deploying ClickHouse" },
  { kind: "default", text: "     ✓ ClickHouse ready — localhost:8123" },
  { kind: "spinner", text: "     🔄 Deploying Anvil" },
  { kind: "default", text: "     ✓ Anvil ready — localhost:8545" },
  { kind: "spinner", text: "     🔄 Deploying eRPC" },
  { kind: "default", text: "     ✓ eRPC ready — localhost:4000" },
  { kind: "spinner", text: "     🔄 Deploying rindexer" },
  { kind: "default", text: "     ✓ 🦀rindexer ready — localhost:18080" },
  { kind: "default", text: "     ✓ All health checks passed" },
  { kind: "headlineBlue", text: "   🏰 ✅ Local stack ready — 1m 41s" },
  { kind: "default", text: "      👉🏻 Anvil         http://localhost:8545" },
  { kind: "default", text: "      👉🏻 eRPC          http://localhost:4000" },
  { kind: "default", text: "      👉🏻 ClickHouse    http://localhost:8123" },
  { kind: "default", text: "      👉🏻 🦀rindexer    http://localhost:18080" },
  { kind: "default", text: "      Chain ID: 31337 (Anvil fresh)" },
  { kind: "default", text: "      👉🏻 Status        evm-cloud local status" },
  { kind: "default", text: "      👉🏻 Tear down     evm-cloud local down" },
];

const SERVICES = [
  { title: "Anvil", subtitle: "EVM simulator (8545)" },
  { title: "eRPC", subtitle: "RPC proxy (4000)" },
  { title: "rindexer", subtitle: "indexer + metrics (18080)" },
  { title: "ClickHouse", subtitle: "query endpoint (8123)" },
];

const styles = {
  root: {
    margin: "18px 0 26px",
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
    gridTemplateColumns: "1.45fr 1fr",
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
  card: {
    border: "1px solid #334155",
    background: "linear-gradient(180deg, #111827 0%, #0f172a 100%)",
    borderRadius: "10px",
    padding: "10px 12px",
    marginBottom: "8px",
    boxShadow: "0 6px 16px rgba(0,0,0,0.22)",
  },
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

export function LocalDevCliShowcase() {
  const [mode, setMode] = useState<Mode>("fork");
  const [visibleCount, setVisibleCount] = useState(0);

  const lines = useMemo(() => (mode === "fork" ? FORK_LINES : FRESH_LINES), [mode]);

  useEffect(() => {
    setVisibleCount(0);
    let cancelled = false;
    let timeout: ReturnType<typeof setTimeout>;

    const tick = (idx: number) => {
      if (cancelled) return;
      if (idx <= lines.length) {
        setVisibleCount(idx);
        timeout = setTimeout(() => tick(idx + 1), idx === 0 ? 320 : 190);
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
        <div style={styles.title}>Local CLI Walkthrough (simulated)</div>
        <div style={styles.chips}>
          <button type="button" style={styles.chip(mode === "fork")} onClick={() => setMode("fork")}>
            Fork (default)
          </button>
          <button type="button" style={styles.chip(mode === "fresh")} onClick={() => setMode("fresh")}>
            Fresh (31337)
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
          <div style={styles.sideTitle}>Local stack services</div>
          {SERVICES.map((item) => (
            <div key={item.title} style={styles.card}>
              <div style={styles.cardTitle}>{item.title}</div>
              <div style={styles.cardSub}>{item.subtitle}</div>
            </div>
          ))}
          <div style={styles.foot}>Matches the `evm-cloud local` workflow in this guide.</div>
        </div>
      </div>
    </div>
  );
}
