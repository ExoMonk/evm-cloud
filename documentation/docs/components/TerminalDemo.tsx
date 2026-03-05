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
  { kind: "cmd", text: "evm-cloud apply --dry-run" },
  { kind: "default", text: "     ✓ Ran terraform plan" },
  { kind: "headlineBlue", text: "🏖️ ✅ Dry run complete - 9s" },
  { kind: "default", text: "      👉🏻 Logs: .evm-cloud/logs/terraform-plan-<ts>.log" },
  { kind: "default", text: "      👉🏻 Output: .evm-cloud/logs/terraform-output-<ts>.json" },
  { kind: "blank", text: "" },
  { kind: "cmd", text: "evm-cloud apply" },
  { kind: "default", text: "     ✓ Ran terraform plan" },
  { kind: "default", text: "     ✔ Apply these changes? (y/N) · yes" },
  { kind: "spinner", text: "     🔄 Terraforming......" },
  { kind: "default", text: "     ✓ Ran terraform apply" },
  { kind: "headlineBlue", text: "🏰 ✅ Infrastructure deployed - 3m 12s" },
  { kind: "default", text: "      👉🏻 Logs: .evm-cloud/logs/terraform-apply-<ts>.log" },
  { kind: "default", text: "      👉🏻 Output: .evm-cloud/logs/terraform-output-<ts>.json" },
  { kind: "blank", text: "" },
  { kind: "cmd", text: "evm-cloud destroy --yes" },
  { kind: "warn", text: "     🚧 running destroy in interactive mode" },
  { kind: "default", text: "     ✔ Destroy infrastructure? (y/N) · yes" },
  { kind: "spinner", text: "     🔄 Terraforming......" },
  { kind: "default", text: "     ✓ Ran terraform destroy" },
  { kind: "headlineRed", text: "🏰 🚀 Destroy complete - 55s" },
];

const CI_LINES: DemoLine[] = [
  { kind: "cmd", text: "evm-cloud init" },
  { kind: "headlineBlue", text: "🏰 ✅ Project initialized" },
  { kind: "blank", text: "" },
  { kind: "cmd", text: "evm-cloud apply --auto-approve" },
  { kind: "default", text: "     ✓ Ran terraform plan" },
  { kind: "spinner", text: "     🔄 Terraforming......" },
  { kind: "default", text: "     ✓ Ran terraform apply" },
  { kind: "headlineBlue", text: "🏰 ✅ Infrastructure deployed - 3m 05s" },
  { kind: "default", text: "      👉🏻 Logs: .evm-cloud/logs/terraform-apply-<ts>.log" },
  { kind: "default", text: "      👉🏻 Output: .evm-cloud/logs/terraform-output-<ts>.json" },
  { kind: "blank", text: "" },
  { kind: "cmd", text: "evm-cloud destroy --yes --auto-approve" },
  { kind: "warn", text: "     🚧 running destroy in non-interactive mode" },
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
    minHeight: "410px",
    fontSize: "14px",
    lineHeight: 1.65,
  },
  line: {
    whiteSpace: "pre-wrap" as const,
    minHeight: "1.45em",
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

const TYPING_SPEED = 26;
const LINE_STAGGER = 145;
const COMMAND_PAUSE = 340;
const RESTART_DELAY = 4200;

const STAGE_LINE_INDEX = {
  init: 1,
  dryRun: 5,
  apply: 14,
  destroy: 23,
} as const;

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
  const status = {
    init: visibleLines > STAGE_LINE_INDEX.init,
    dryRun: visibleLines > STAGE_LINE_INDEX.dryRun,
    apply: visibleLines > STAGE_LINE_INDEX.apply,
    destroy: visibleLines > (mode === "interactive" ? STAGE_LINE_INDEX.destroy : 15),
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
            {[
              { label: "Init complete", on: status.init },
              { label: "Plan validated", on: status.dryRun },
              { label: "Infra applied", on: status.apply },
              { label: "Destroy verified", on: status.destroy },
            ].map((item) => (
              <div
                key={item.label}
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
