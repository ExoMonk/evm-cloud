import type { Dispatch } from "react";
import type { BuilderState, BuilderAction } from "../../../lib/configSchema.ts";

interface Props {
  state: BuilderState;
  dispatch: Dispatch<BuilderAction>;
}

export function ProjectStep({ state, dispatch }: Props) {
  return (
    <div>
      <label className="block text-[11px] uppercase tracking-[0.15em] text-[var(--color-text-muted)] mb-2">
        project name
      </label>
      <input
        type="text"
        value={state.projectName}
        onChange={(e) => dispatch({ type: "SET_PROJECT_NAME", name: e.target.value })}
        placeholder="evm-cloud-demo"
        className="w-full px-3 py-2.5 bg-transparent border border-[var(--color-border)] text-[13px] text-[var(--color-text)] placeholder-[var(--color-text-muted)] focus:outline-none focus:border-[var(--color-accent)]/50 transition-colors"
      />
      {!state.projectName.trim() && (
        <p className="text-[11px] text-[var(--color-error)] mt-1.5">project name is required.</p>
      )}
    </div>
  );
}
