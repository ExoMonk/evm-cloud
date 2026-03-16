import { useState, useMemo } from "react";
import type { BuilderState } from "../../lib/configSchema.ts";
import { generateToml } from "../../lib/tomlGenerator.ts";
import { validate } from "../../lib/configValidator.ts";
import { estimateCost } from "../../lib/costData.ts";
import { generateRindexerYaml } from "../../lib/rindexerGenerator.ts";
import { generateErpcYaml } from "../../lib/erpcGenerator.ts";
import { CornerCard } from "../ui/CornerCard.tsx";
import { SectionHeader } from "../ui/SectionHeader.tsx";

type Tab = "toml" | "rindexer" | "erpc";

interface Props {
  state: BuilderState;
}

export function PreviewPanel({ state }: Props) {
  const [activeTab, setActiveTab] = useState<Tab>("toml");

  const toml = useMemo(() => generateToml(state), [state]);
  const rindexerYaml = useMemo(() => generateRindexerYaml(state), [state]);
  const erpcYaml = useMemo(() => generateErpcYaml(state), [state]);
  const issues = useMemo(() => validate(state), [state]);
  const cost = useMemo(() => estimateCost(state), [state]);

  const errors = issues.filter((i) => i.severity === "error");
  const warnings = issues.filter((i) => i.severity === "warning");

  const tabs: { id: Tab; label: string }[] = [
    { id: "toml", label: "evm-cloud.toml" },
    { id: "rindexer", label: "rindexer.yaml" },
    { id: "erpc", label: "erpc.yaml" },
  ];

  const getContent = (tab: Tab): string => {
    switch (tab) {
      case "toml": return toml;
      case "rindexer": return rindexerYaml;
      case "erpc": return erpcYaml;
    }
  };

  return (
    <div className="space-y-5">
      <SectionHeader label="preview" />

      {/* Config file tabs + code preview */}
      <CornerCard accent className="p-0 overflow-hidden">
        <div className="flex items-center border-b border-[var(--color-border)]">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`
                px-3 py-2.5 text-[11px] whitespace-nowrap transition-colors
                ${activeTab === tab.id
                  ? "text-[var(--color-accent)] border-b border-[var(--color-accent)]"
                  : "text-[var(--color-text-muted)] hover:text-[var(--color-text-dim)]"
                }
              `}
            >
              {tab.label}
            </button>
          ))}
        </div>
        <pre className="p-4 text-[12px] leading-relaxed text-[var(--color-text-dim)] overflow-auto max-h-72 scrollbar-none">
          {getContent(activeTab)}
        </pre>
      </CornerCard>

      {/* Cost estimate */}
      {state.infraProfile && (
        <CornerCard className="p-5">
          <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-3">
            // cost estimate
          </p>
          <p className="text-[20px] font-light text-[var(--color-text)]">
            {cost.monthlyMin === 0 && cost.monthlyMax === 0
              ? "VPS cost only"
              : `~$${cost.monthlyMin}–${cost.monthlyMax}/mo`}
          </p>
          <div className="mt-3 space-y-1.5">
            {cost.breakdown.map((line, i) => (
              <div key={i} className="flex justify-between text-[11px]">
                <span className="text-[var(--color-text-muted)]">{line.component}</span>
                <span className="text-[var(--color-text-dim)]">{line.cost}</span>
              </div>
            ))}
          </div>
          {cost.warnings.length > 0 && (
            <div className="mt-3 pt-3 border-t border-[var(--color-border)] space-y-1.5">
              {cost.warnings.map((w, i) => (
                <p
                  key={i}
                  className={`text-[11px] ${
                    w.severity === "warning" ? "text-[var(--color-warning)]" : "text-[var(--color-text-muted)]"
                  }`}
                >
                  {w.severity === "warning" ? "⚠ " : ""}
                  {w.message}
                </p>
              ))}
            </div>
          )}
        </CornerCard>
      )}

      {/* Validation */}
      <CornerCard accent={errors.length === 0} className="p-5">
        <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-3">
          // validation
        </p>
        {errors.length === 0 && warnings.length === 0 ? (
          <p className="text-[12px] text-[var(--color-accent)]">● all checks pass</p>
        ) : (
          <div className="space-y-1.5">
            {errors.map((e, i) => (
              <p key={`err-${i}`} className="text-[12px] text-[var(--color-error)]">
                ✕ {e.message}
              </p>
            ))}
            {warnings.map((w, i) => (
              <p key={`warn-${i}`} className="text-[12px] text-[var(--color-warning)]">
                ⚠ {w.message}
              </p>
            ))}
          </div>
        )}
      </CornerCard>
    </div>
  );
}
