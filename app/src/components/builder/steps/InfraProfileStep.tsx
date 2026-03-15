import type { Dispatch } from "react";
import type { BuilderState, BuilderAction } from "../../../lib/configSchema.ts";
import { INFRA_PROFILES, AWS_REGIONS } from "../../../lib/configSchema.ts";
import { CornerCard } from "../../ui/CornerCard.tsx";

interface Props {
  state: BuilderState;
  dispatch: Dispatch<BuilderAction>;
}

export function InfraProfileStep({ state, dispatch }: Props) {
  return (
    <div className="space-y-5">
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        {INFRA_PROFILES.map((profile) => {
          const isSelected = state.infraProfile === profile.id;
          return (
            <CornerCard
              key={profile.id}
              accent={isSelected}
              hover
              className={`cursor-pointer p-4 ${isSelected ? "bg-[var(--color-accent-dim)]" : ""}`}
            >
              <button
                onClick={() => dispatch({ type: "SET_INFRA_PROFILE", profile: profile.id })}
                className="w-full text-left"
              >
                <div className="flex items-center justify-between">
                  <span className={`text-[12px] font-medium ${isSelected ? "text-[var(--color-accent)]" : "text-[var(--color-text)]"}`}>
                    {profile.name}
                  </span>
                  <span className="text-[11px] text-[var(--color-accent)]">
                    {profile.costRange}
                  </span>
                </div>
                <p className="text-[11px] text-[var(--color-text-muted)] mt-1.5">{profile.description}</p>
                <p className="text-[10px] text-[var(--color-text-muted)]/60 mt-1.5">
                  {profile.provider} / {profile.engine}
                </p>
              </button>
            </CornerCard>
          );
        })}
      </div>

      {/* AWS-specific fields */}
      {state.provider === "aws" && (
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="block text-[11px] uppercase tracking-[0.15em] text-[var(--color-text-muted)] mb-2">
              region
            </label>
            <select
              value={state.region}
              onChange={(e) => dispatch({ type: "SET_REGION", region: e.target.value })}
              className="w-full px-3 py-2.5 bg-transparent border border-[var(--color-border)] text-[13px] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-accent)]/50 transition-colors"
            >
              {AWS_REGIONS.map((r) => (
                <option key={r} value={r} className="bg-[var(--color-bg)]">{r}</option>
              ))}
            </select>
          </div>
          <div>
            <label className="block text-[11px] uppercase tracking-[0.15em] text-[var(--color-text-muted)] mb-2">
              instance type
            </label>
            <input
              type="text"
              value={state.instanceType}
              onChange={(e) => dispatch({ type: "SET_INSTANCE_TYPE", instanceType: e.target.value })}
              placeholder="t3.small"
              className="w-full px-3 py-2.5 bg-transparent border border-[var(--color-border)] text-[13px] text-[var(--color-text)] placeholder-[var(--color-text-muted)] focus:outline-none focus:border-[var(--color-accent)]/50 transition-colors"
            />
          </div>
        </div>
      )}
    </div>
  );
}
