import type { Dispatch } from "react";
import type { BuilderState, BuilderAction } from "../../../lib/configSchema.ts";
import { CornerCard } from "../../ui/CornerCard.tsx";

interface Props {
  state: BuilderState;
  dispatch: Dispatch<BuilderAction>;
}

export function ReviewStep({ state, dispatch: _dispatch }: Props) {
  return (
    <div className="space-y-5">
      {/* Summary table */}
      <CornerCard className="p-5">
        <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-3">
          // summary
        </p>
        <div className="space-y-2 text-[12px]">
          <Row label="project" value={state.projectName} />
          <Row label="infra" value={state.infraProfile ?? "—"} />
          <Row label="provider" value={`${state.provider} / ${state.computeEngine}`} />
          {state.provider === "aws" && (
            <>
              <Row label="region" value={state.region} />
              <Row label="instance" value={state.instanceType} />
            </>
          )}
          <Row label="database" value={state.databaseProfile} />
          <Row label="chains" value={state.chains.join(", ") || "—"} accent={state.chains.length > 0} />
          <Row label="ingress" value={state.ingressMode} />
          {state.domain && <Row label="domain" value={state.domain} />}
          <Row label="secrets" value={state.secretsMode} />
          <Row label="workload" value={state.workloadMode} />
        </div>
      </CornerCard>

      {/* Advanced sections */}
      <div>
        <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-3">
          // advanced
        </p>
        <div className="space-y-1.5">
          {["secrets", "monitoring", "networking", "state backend", "streaming", "containers", "extra env"].map(
            (section) => (
              <div
                key={section}
                className="px-4 py-2.5 border border-[var(--color-border)] text-[11px] uppercase tracking-[0.15em] text-[var(--color-text-muted)] cursor-pointer hover:border-[var(--color-border-hover)] transition-colors"
              >
                ▸ {section}
              </div>
            )
          )}
        </div>
      </div>

      {/* Deploy workflow */}
      <CornerCard accent className="p-5">
        <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-3">
          // after download
        </p>
        {state.workloadMode === "external" ? (
          <div className="space-y-1.5 text-[12px] text-[var(--color-text-dim)]">
            <p><span className="text-[var(--color-accent)]">$</span> cd {state.projectName}</p>
            <p><span className="text-[var(--color-accent)]">$</span> make plan</p>
            <p><span className="text-[var(--color-accent)]">$</span> make apply</p>
            <p className="text-[var(--color-text-muted)] mt-2">then run the deployer script for workloads (two-phase).</p>
          </div>
        ) : (
          <div className="space-y-1.5 text-[12px] text-[var(--color-text-dim)]">
            <p><span className="text-[var(--color-accent)]">$</span> cd {state.projectName}</p>
            <p><span className="text-[var(--color-accent)]">$</span> make plan</p>
            <p><span className="text-[var(--color-accent)]">$</span> make apply</p>
          </div>
        )}
      </CornerCard>

    </div>
  );
}

function Row({ label, value, accent = false }: { label: string; value: string; accent?: boolean }) {
  return (
    <div className="flex justify-between items-baseline">
      <span className="text-[var(--color-text-muted)]">{label}</span>
      <span className={accent ? "text-[var(--color-accent)]" : "text-[var(--color-text-dim)]"}>
        {value}
      </span>
    </div>
  );
}
