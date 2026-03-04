import React, { useEffect, useState, useRef, useCallback } from "react";

// ─── Animation Data ─────────────────────────────────────────────

type LineType = "command" | "output" | "blank" | "status" | "url";

type Line = {
  type: LineType;
  text: string;
  color?: string;
  delay?: number; // ms before this line appears
};

const LINES: Line[] = [
  // Command 1
  { type: "command", text: "evm-cloud init --example k3s_clickhouse" },
  { type: "blank", text: "" },
  { type: "output", text: "  ✓ Created evm-cloud.toml", color: "#10b981" },
  {
    type: "output",
    text: "  ✓ Linked rindexer.yaml (3 contracts, 12 events)",
    color: "#10b981",
  },
  { type: "output", text: "  ✓ Terraform initialized", color: "#10b981" },
  { type: "blank", text: "" },
  // Command 2
  { type: "command", text: "evm-cloud apply" },
  { type: "blank", text: "" },
  {
    type: "output",
    text: "  ▸ Planning infrastructure...",
    color: "#0ea5e9",
  },
  {
    type: "output",
    text: "  ✓ VPC + networking           4s",
    color: "#10b981",
  },
  {
    type: "output",
    text: "  ✓ k3s cluster (3 nodes)      2m 14s",
    color: "#10b981",
  },
  { type: "output", text: "  ✓ eRPC proxy                 8s", color: "#10b981" },
  {
    type: "output",
    text: "  ✓ rindexer indexer            12s",
    color: "#10b981",
  },
  {
    type: "output",
    text: "  ✓ ClickHouse connected        3s",
    color: "#10b981",
  },
  { type: "blank", text: "" },
  { type: "status", text: "  🟢 Stack deployed — 2m 41s" },
  { type: "url", text: "     eRPC:     https://rpc.example.com" },
  { type: "url", text: "     Grafana:  https://grafana.example.com" },
  { type: "url", text: "     SSH:      ssh ubuntu@203.0.113.42" },
];

const TYPING_SPEED = 35; // ms per character
const LINE_STAGGER = 120; // ms between output lines
const COMMAND_PAUSE = 400; // ms pause after typing a command
const RESTART_DELAY = 5000; // ms before looping

// ─── Styles ─────────────────────────────────────────────────────

const s = {
  wrapper: {
    display: "flex",
    justifyContent: "center",
    marginTop: "24px",
    marginBottom: "28px",
    width: "min(90vw, 960px)",
  },
  terminal: {
    width: "100%",
    borderRadius: "14px",
    overflow: "hidden",
    border: "1px solid #30363d",
    backgroundColor: "#0d1117",
    fontFamily:
      "'SF Mono', 'Fira Code', 'Cascadia Code', 'JetBrains Mono', monospace",
    fontSize: "15px",
    lineHeight: 1.7,
  },
  titleBar: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    padding: "10px 14px",
    backgroundColor: "#161b22",
    borderBottom: "1px solid #30363d",
  },
  dot: (color: string) => ({
    width: "10px",
    height: "10px",
    borderRadius: "50%",
    backgroundColor: color,
  }),
  titleText: {
    marginLeft: "8px",
    fontSize: "12px",
    color: "#8b949e",
    fontWeight: 500 as const,
  },
  body: {
    padding: "24px 32px",
    minHeight: "400px",
    overflow: "hidden",
  },
  line: {
    whiteSpace: "pre" as const,
    minHeight: "1.6em",
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
    height: "15px",
    backgroundColor: "#e6edf3",
    verticalAlign: "text-bottom",
    marginLeft: "1px",
  },
};

// ─── Component ──────────────────────────────────────────────────

export function TerminalDemo() {
  const [visibleLines, setVisibleLines] = useState<number>(0);
  const [typedChars, setTypedChars] = useState<number>(0);
  const [isTyping, setIsTyping] = useState<boolean>(false);
  const [showCursor, setShowCursor] = useState<boolean>(true);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const prefersReducedMotion = useRef(false);

  // Check reduced motion preference
  useEffect(() => {
    if (typeof window !== "undefined") {
      const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
      prefersReducedMotion.current = mq.matches;
      if (mq.matches) {
        // Show everything immediately
        setVisibleLines(LINES.length);
        setIsTyping(false);
        setShowCursor(false);
      }
    }
  }, []);

  // Cursor blink
  useEffect(() => {
    if (prefersReducedMotion.current) return;
    const interval = setInterval(() => {
      setShowCursor((prev) => !prev);
    }, 530);
    return () => clearInterval(interval);
  }, []);

  const clearTimeouts = useCallback(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
  }, []);

  // Animation loop
  useEffect(() => {
    if (prefersReducedMotion.current) return;

    let cancelled = false;
    let currentTimeout: ReturnType<typeof setTimeout>;

    const schedule = (fn: () => void, ms: number) => {
      currentTimeout = setTimeout(() => {
        if (!cancelled) fn();
      }, ms);
      timeoutRef.current = currentTimeout;
    };

    const animateLine = (lineIdx: number) => {
      if (cancelled || lineIdx >= LINES.length) {
        // Done — wait then restart
        setIsTyping(false);
        schedule(() => {
          setVisibleLines(0);
          setTypedChars(0);
          setIsTyping(false);
          schedule(() => animateLine(0), 600);
        }, RESTART_DELAY);
        return;
      }

      const line = LINES[lineIdx];

      if (line.type === "command") {
        // Type command character by character
        setVisibleLines(lineIdx + 1);
        setIsTyping(true);
        setTypedChars(0);

        const typeChar = (charIdx: number) => {
          if (cancelled) return;
          if (charIdx >= line.text.length) {
            // Done typing this command
            setTypedChars(line.text.length);
            setIsTyping(false);
            schedule(() => animateLine(lineIdx + 1), COMMAND_PAUSE);
            return;
          }
          setTypedChars(charIdx + 1);
          schedule(() => typeChar(charIdx + 1), TYPING_SPEED);
        };

        schedule(() => typeChar(0), 200);
      } else {
        // Output line — appear instantly with stagger
        setVisibleLines(lineIdx + 1);
        schedule(() => animateLine(lineIdx + 1), LINE_STAGGER);
      }
    };

    // Start animation after a brief initial delay
    schedule(() => animateLine(0), 800);

    return () => {
      cancelled = true;
      clearTimeouts();
    };
  }, [clearTimeouts]);

  const getLineColor = (line: Line): string => {
    if (line.color) return line.color;
    switch (line.type) {
      case "status":
        return "#10b981";
      case "url":
        return "#58a6ff";
      default:
        return "#8b949e";
    }
  };

  const renderLine = (line: Line, idx: number) => {
    if (idx >= visibleLines) return null;

    if (line.type === "blank") {
      return <div key={idx} style={s.line}>&nbsp;</div>;
    }

    if (line.type === "command") {
      const isCurrentlyTyping = isTyping && idx === visibleLines - 1;
      const displayText = isCurrentlyTyping
        ? line.text.slice(0, typedChars)
        : line.text;

      return (
        <div key={idx} style={s.line}>
          <span style={s.prompt}>$ </span>
          <span style={s.commandText}>{displayText}</span>
          {isCurrentlyTyping && showCursor && (
            <span style={s.cursor} />
          )}
        </div>
      );
    }

    // Output, status, or URL lines
    const color = getLineColor(line);
    return (
      <div key={idx} style={{ ...s.line, color }}>
        {line.text}
      </div>
    );
  };

  return (
    <div style={s.wrapper}>
      <div style={s.terminal}>
        <div style={s.titleBar}>
          <span style={s.dot("#ff5f57")} />
          <span style={s.dot("#febc2e")} />
          <span style={s.dot("#28c840")} />
          <span style={s.titleText}>evm-cloud</span>
        </div>
        <div style={s.body}>
          {LINES.map((line, idx) => renderLine(line, idx))}
        </div>
      </div>
    </div>
  );
}
