import React, { useEffect, useMemo, useRef, useState } from "react";

type Mode = "interactive" | "ci";
type LineKind = "cmd" | "headlineBlue" | "headlineRed" | "default" | "warn" | "spinner" | "blank";

type DemoLine = {
  kind: LineKind;
  text: string;
};

const INTERACTIVE_LINES: DemoLine[] = [
  { kind: "cmd", text: "evm-cloud init" },
  { kind: "headlineBlue", text: "🏰 ✅ Project initialized" },
  { kind: "blank", text: "" },
  { kind: "cmd", text: "evm-cloud deploy" },
  { kind: "headlineBlue", text: "🏰 ⚒️ Deploying sandboxx" },
  { kind: "default", text: "   ▸ Phase 1/2 — Deploying Infrastructure" },
  { kind: "default", text: "     ✓ Ran terraform plan" },
  { kind: "default", text: "     ✔ Apply these changes? (y/N) · y" },
  { kind: "default", text: "     ✓ Ran terraform apply" },
  { kind: "default", text: "     ✓ VPC + networking" },
  { kind: "default", text: "     ✓ k3s cluster (2 nodes)" },
  { kind: "default", text: "   ▸ Phase 2/2 — Deploying Workload" },
  { kind: "default", text: "     🛟 ClusterSecretStore: evm-cloud-k3s-prod-aws-sm" },
  { kind: "default", text: "     ✔ Cloudflare origin TLS secret created" },
  { kind: "default", text: "     ✔ kube-prometheus-stack" },
  { kind: "default", text: "     🛟 Dashboards deployed" },
  { kind: "default", text: "     🛟 eRPC: evm-cloud-k3s-prod-erpc" },
  { kind: "default", text: "     🛟 rindexer #1: evm-cloud-k3s-prod-indexer" },
  { kind: "default", text: "     🛟 rindexer #2: evm-cloud-k3s-prod-backfill" },
  { kind: "headlineBlue", text: "🏰 ✅ Deploy complete - 3m 12s" },
  { kind: "default", text: "      👉🏻 Server       ubuntu@<>" },
  { kind: "default", text: "      👉🏻 Grafana      https://grafana.evm-cloud.xyz" },
  { kind: "default", text: "      👉🏻 Status       evm-cloud status" },
  { kind: "blank", text: "" },
  { kind: "cmd", text: "evm-cloud destroy --yes" },
  { kind: "default", text: "     🛟 Pods tore down" },
  { kind: "default", text: "     ✔ Destroy infrastructure? (y/N) · y" },
  { kind: "spinner", text: "     🔄 Terraforming......" },
  { kind: "default", text: "     ✓ Ran terraform destroy" },
  { kind: "headlineRed", text: "🏰 🚀 Destroy complete - 55s" },
];

const CI_LINES: DemoLine[] = [
  { kind: "cmd", text: "evm-cloud init" },
  { kind: "headlineBlue", text: "🏰 ✅ Project initialized" },
  { kind: "blank", text: "" },
  { kind: "cmd", text: "evm-cloud deploy --auto-approve" },
  { kind: "headlineBlue", text: "🏰 ⚒️ Deploying sandbox" },
  { kind: "default", text: "   ▸ Phase 1/2 — Deploying Infrastructure" },
  { kind: "default", text: "     ✓ Ran terraform apply" },
  { kind: "default", text: "     ✓ VPC + networking" },
  { kind: "default", text: "     ✓ k3s cluster (2 nodes)" },
  { kind: "default", text: "   ▸ Phase 2/2 — Deploying Workload" },
  { kind: "default", text: "     🛟 ClusterSecretStore: evm-cloud-k3s-prod-aws-sm" },
  { kind: "default", text: "     ✔ Cloudflare origin TLS secret created" },
  { kind: "default", text: "     ✔ kube-prometheus-stack" },
  { kind: "default", text: "     🛟 Dashboards deployed" },
  { kind: "default", text: "     🛟 eRPC: evm-cloud-k3s-prod-erpc" },
  { kind: "default", text: "     🛟 rindexer #1: evm-cloud-k3s-prod-indexer" },
  { kind: "default", text: "     🛟 rindexer #2: evm-cloud-k3s-prod-backfill" },
  { kind: "headlineBlue", text: "🏰 ✅ Deploy complete - 3m 6s" },
  { kind: "default", text: "      👉🏻 Server       ubuntu@<>" },
  { kind: "default", text: "      👉🏻 Grafana      https://grafana.evm-cloud.xyz" },
  { kind: "default", text: "      👉🏻 Status       evm-cloud status" },
  { kind: "blank", text: "" },
  { kind: "cmd", text: "evm-cloud destroy --yes --auto-approve" },
  { kind: "default", text: "     🛟 Pods tore down" },
  { kind: "spinner", text: "     🔄 Terraforming......" },
  { kind: "default", text: "     ✓ Ran terraform destroy" },
  { kind: "headlineRed", text: "🏰 🚀 Destroy complete - 48s" },
];

const s = {
  wrapper: {
    display: "flex",
    justifyContent: "center",
    marginTop: "24px",
    marginBottom: "28px",
    width: "min(92vw, 1040px)",
    textAlign: "left" as const,
  },
  terminal: {
    width: "100%",
    borderRadius: "14px",
    overflow: "hidden",
    border: "1px solid #30363d",
    backgroundColor: "#0d1117",
    boxShadow: "0 20px 48px rgba(0,0,0,0.35)",
    fontFamily:
      "'SF Mono', 'Fira Code', 'Cascadia Code', 'JetBrains Mono', monospace",
  },
  titleBar: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "10px",
    padding: "10px 14px",
    backgroundColor: "#161b22",
    borderBottom: "1px solid #30363d",
    flexWrap: "wrap" as const,
  },
  titleLeft: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
  },
  dot: (color: string) => ({
    width: "10px",
    height: "10px",
    borderRadius: "50%",
    backgroundColor: color,
  }),
  pulse: {
    width: "8px",
    height: "8px",
    borderRadius: "50%",
    backgroundColor: "#22c55e",
    boxShadow: "0 0 0 0 rgba(34,197,94,0.55)",
    animation: "pulse-demo 1.8s infinite",
  },
  titleText: {
    marginLeft: "8px",
    fontSize: "12px",
    color: "#8b949e",
    fontWeight: 500 as const,
  },
  chips: {
    display: "flex",
    gap: "8px",
  },
  chip: (active: boolean) => ({
    border: "1px solid #30363d",
    borderRadius: "999px",
    padding: "4px 10px",
    fontSize: "11px",
    cursor: "pointer",
    background: active ? "#2563eb33" : "#0d1117",
    color: active ? "#93c5fd" : "#9ca3af",
  }),
  body: {
    padding: "20px 22px",
    minHeight: "680px",
    fontSize: "14px",
    lineHeight: 1.65,
    textAlign: "left" as const,
  },
  line: {
    whiteSpace: "pre-wrap" as const,
    minHeight: "1.45em",
    textAlign: "left" as const,
  },
  prompt: {
    color: "#8b949e",
  },
  commandText: {
    color: "#e6edf3",
  },
  cursor: {
    display: "inline-block",
    width: "8px",
    height: "14px",
    backgroundColor: "#e6edf3",
    verticalAlign: "text-bottom",
    marginLeft: "1px",
  },
} as const;

type InfraNode = {
  id: string;
  label: string;
  icon: string;
  desc: string;
};

const INFRA_NODES: InfraNode[] = [
  { id: "vpc",     label: "VPC",     icon: "🌐", desc: "Networking + SGs" },
  { id: "k3s",     label: "k3s",     icon: "⚡", desc: "2-node cluster" },
  { id: "secrets", label: "Secrets", icon: "🔐", desc: "AWS Secrets Manager" },
];

const WORKLOAD_NODES: InfraNode[] = [
  { id: "monitoring", label: "Monitoring", icon: "📊", desc: "Prometheus + Grafana" },
  { id: "erpc",       label: "eRPC",       icon: "🔀", desc: "RPC failover proxy" },
  { id: "rindexer",   label: "rindexer",   icon: "🦀", desc: "EVM event indexer" },
];

const ALL_NODES = [...INFRA_NODES, ...WORKLOAD_NODES];

/* Line index at which each node lights up (visibleLines > threshold) */
const NODE_THRESHOLDS: Record<Mode, Record<string, number>> = {
  interactive: { vpc: 9, k3s: 10, secrets: 12, monitoring: 14, erpc: 16, rindexer: 17 },
  ci:          { vpc: 7, k3s: 8,  secrets: 10, monitoring: 12, erpc: 14, rindexer: 15 },
};

const TYPING_SPEED = 23;
const LINE_STAGGER = 145;
const COMMAND_PAUSE = 360;
const RESTART_DELAY = 4500;

function lineColor(kind: LineKind): string {
  switch (kind) {
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

export function TerminalDemo() {
  const [mode, setMode] = useState<Mode>("interactive");
  const [visibleLines, setVisibleLines] = useState(0);
  const [typedChars, setTypedChars] = useState(0);
  const [isTyping, setIsTyping] = useState(false);
  const [showCursor, setShowCursor] = useState(true);
  const [isCompact, setIsCompact] = useState(false);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const prefersReducedMotion = useRef(false);

  const lines = useMemo(
    () => (mode === "interactive" ? INTERACTIVE_LINES : CI_LINES),
    [mode]
  );

  const progressPct = Math.min(100, Math.round((visibleLines / lines.length) * 100));

  const activeNodeIds = useMemo(() => {
    const thresholds = NODE_THRESHOLDS[mode];
    return new Set(
      ALL_NODES.filter((n) => visibleLines > (thresholds[n.id] ?? Infinity)).map((n) => n.id)
    );
  }, [mode, visibleLines]);

  /* Stage flags for the status cards */
  const stageThresholds = { interactive: { init: 1, infra: 5, workload: 11, destroy: 25 }, ci: { init: 1, infra: 5, workload: 9, destroy: 19 } } as const;
  const stage = stageThresholds[mode];
  const status = {
    init: visibleLines > stage.init,
    infra: visibleLines > stage.infra,
    workload: visibleLines > stage.workload,
    destroy: visibleLines > stage.destroy,
  };

  useEffect(() => {
    if (typeof window !== "undefined") {
      const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
      prefersReducedMotion.current = mq.matches;
      if (mq.matches) {
        setVisibleLines(lines.length);
        setIsTyping(false);
        setShowCursor(false);
      }
    }
  }, [lines.length]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const mq = window.matchMedia("(max-width: 1080px)");
    const update = () => setIsCompact(mq.matches);
    update();
    if (typeof mq.addEventListener === "function") {
      mq.addEventListener("change", update);
      return () => mq.removeEventListener("change", update);
    }
    mq.addListener(update);
    return () => mq.removeListener(update);
  }, []);

  useEffect(() => {
    if (prefersReducedMotion.current) return;
    const interval = setInterval(() => setShowCursor((v) => !v), 520);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    if (prefersReducedMotion.current) return;

    let cancelled = false;

    const schedule = (fn: () => void, ms: number) => {
      timeoutRef.current = setTimeout(() => {
        if (!cancelled) fn();
      }, ms);
    };

    const run = (lineIdx: number) => {
      if (cancelled) return;

      if (lineIdx >= lines.length) {
        setIsTyping(false);
        schedule(() => {
          setVisibleLines(0);
          setTypedChars(0);
          run(0);
        }, RESTART_DELAY);
        return;
      }

      const line = lines[lineIdx];

      if (line.kind === "cmd") {
        setVisibleLines(lineIdx + 1);
        setIsTyping(true);
        setTypedChars(0);

        const typeChar = (charIdx: number) => {
          if (cancelled) return;
          if (charIdx >= line.text.length) {
            setTypedChars(line.text.length);
            setIsTyping(false);
            schedule(() => run(lineIdx + 1), COMMAND_PAUSE);
            return;
          }
          setTypedChars(charIdx + 1);
          schedule(() => typeChar(charIdx + 1), TYPING_SPEED);
        };

        schedule(() => typeChar(0), 120);
      } else {
        setVisibleLines(lineIdx + 1);
        schedule(() => run(lineIdx + 1), LINE_STAGGER);
      }
    };

    setVisibleLines(0);
    setTypedChars(0);
    setIsTyping(false);
    schedule(() => run(0), 420);

    return () => {
      cancelled = true;
      if (timeoutRef.current) clearTimeout(timeoutRef.current);
    };
  }, [lines]);

  const renderLine = (line: DemoLine, idx: number) => {
    if (idx >= visibleLines) return null;

    if (line.kind === "blank") {
      return <div key={idx} style={{ ...s.line, animation: "line-in .22s ease-out" }} aria-hidden="true" />;
    }

    if (line.kind === "cmd") {
      const currentlyTyping = isTyping && idx === visibleLines - 1;
      const displayText = currentlyTyping ? line.text.slice(0, typedChars) : line.text;

      return (
        <div key={idx} style={{ ...s.line, animation: "line-in .22s ease-out" }}>
          <span style={s.prompt}>$ </span>
          <span style={s.commandText}>{displayText}</span>
          {currentlyTyping && showCursor && <span style={s.cursor} />}
        </div>
      );
    }

    return (
      <div key={idx} style={{ ...s.line, color: lineColor(line.kind), animation: "line-in .22s ease-out" }}>
        {line.text}
      </div>
    );
  };

  return (
    <div style={s.wrapper}>
      <style>{`
        @keyframes pulse-demo { 0% { box-shadow: 0 0 0 0 rgba(34,197,94,.5); } 70% { box-shadow: 0 0 0 8px rgba(34,197,94,0); } 100% { box-shadow: 0 0 0 0 rgba(34,197,94,0); } }
        @keyframes line-in { from { opacity: .2; transform: translateY(4px); } to { opacity: 1; transform: translateY(0); } }
        @keyframes shine-in { from { opacity: .4; transform: scale(.98); } to { opacity: 1; transform: scale(1); } }
`}</style>
      <div
        style={{
          display: "grid",
          width: "100%",
          gridTemplateColumns: isCompact ? "minmax(0,1fr)" : "minmax(0,1fr) minmax(220px,260px)",
          gap: "14px",
          alignItems: "stretch",
        }}
      >
        <div style={{ ...s.terminal, animation: "shine-in .42s ease-out" }}>
          <div style={s.titleBar}>
            <div style={s.titleLeft}>
              <span style={s.dot("#ff5f57")} />
              <span style={s.dot("#febc2e")} />
              <span style={s.dot("#28c840")} />
              <span style={s.titleText}>evm-cloud</span>
              <span style={s.pulse} />
            </div>

            <div style={s.chips}>
              <button type="button" style={s.chip(mode === "interactive")} onClick={() => setMode("interactive")}>
                Interactive
              </button>
              <button type="button" style={s.chip(mode === "ci")} onClick={() => setMode("ci")}>
                CI / Non-interactive
              </button>
            </div>
          </div>

          <div style={{ padding: "0 22px", background: "linear-gradient(90deg, rgba(37,99,235,.15), rgba(34,211,238,.08))" }}>
            <div style={{ height: "3px", width: "100%", background: "rgba(148,163,184,.2)", borderRadius: "999px", overflow: "hidden" }}>
              <div
                style={{
                  height: "100%",
                  width: `${progressPct}%`,
                  background: "linear-gradient(90deg,#60a5fa,#22d3ee)",
                  transition: "width 180ms ease",
                  boxShadow: "0 0 10px rgba(34,211,238,.6)",
                }}
              />
            </div>
          </div>

          <div style={{ ...s.body, padding: isCompact ? "22px 20px" : "26px 30px", lineHeight: 1.78, fontSize: "15px" }}>
            {lines.map((line, idx) => renderLine(line, idx))}
          </div>
        </div>

        {!isCompact && (
          <div
            style={{
              borderRadius: "14px",
              border: "1px solid #30363d",
              background: "linear-gradient(180deg,#0d1117,#020817)",
              padding: "14px",
              display: "flex",
              flexDirection: "column",
              gap: "10px",
              animation: "shine-in .52s ease-out",
            }}
          >
            <div style={{ color: "#cbd5e1", fontSize: "12px", fontWeight: 600, letterSpacing: ".03em", textTransform: "uppercase" }}>
              Live Stack Status
            </div>
            {([
              { label: "Init complete", on: status.init, nodes: null },
              { label: "Infra deployed", on: status.infra, nodes: INFRA_NODES },
              { label: "Workload deployed", on: status.workload, nodes: WORKLOAD_NODES },
              { label: "Destroy verified", on: status.destroy, nodes: null },
            ] as const).map((item) => (
              <div key={item.label}>
                <div
                  style={{
                    border: "1px solid #334155",
                    borderRadius: "10px",
                    padding: "10px 11px",
                    background: item.on ? "linear-gradient(90deg, rgba(34,197,94,.18), rgba(34,197,94,.06))" : "rgba(15,23,42,.55)",
                    color: item.on ? "#86efac" : "#94a3b8",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    gap: "10px",
                    transition: "all 180ms ease",
                  }}
                >
                  <span style={{ fontSize: "13px" }}>{item.label}</span>
                  <span
                    style={{
                      width: "8px",
                      height: "8px",
                      borderRadius: "999px",
                      background: item.on ? "#22c55e" : "#475569",
                      boxShadow: item.on ? "0 0 10px rgba(34,197,94,.9)" : "none",
                      transition: "all 180ms ease",
                    }}
                  />
                </div>
                {item.nodes && (
                  <div style={{ display: "flex", flexWrap: "wrap", gap: "4px", padding: "5px 0 0 8px" }}>
                    {item.nodes.map((node) => {
                      const on = activeNodeIds.has(node.id);
                      return (
                        <span
                          key={node.id}
                          style={{
                            display: "inline-flex",
                            alignItems: "center",
                            gap: "3px",
                            padding: "2px 6px",
                            borderRadius: "5px",
                            fontSize: "10px",
                            border: `1px solid ${on ? "#22c55e25" : "#1e293b"}`,
                            background: on ? "rgba(34,197,94,0.06)" : "transparent",
                            color: on ? "#86efac" : "#475569",
                            opacity: on ? 1 : 0.45,
                            transition: "all 250ms ease",
                          }}
                        >
                          <span style={{ fontSize: "9px" }}>{node.icon}</span>
                          {node.label}
                        </span>
                      );
                    })}
                  </div>
                )}
              </div>
            ))}

            <div style={{ marginTop: "auto", borderTop: "1px solid #334155", paddingTop: "10px", color: "#93c5fd", fontSize: "12px" }}>
              Replay loops automatically for quick scan.
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
