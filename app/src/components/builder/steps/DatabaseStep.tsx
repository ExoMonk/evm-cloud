import type { Dispatch } from "react";
import type { BuilderState, BuilderAction, DatabaseProfile } from "../../../lib/configSchema.ts";

interface Props {
  state: BuilderState;
  dispatch: Dispatch<BuilderAction>;
}

const DB_OPTIONS: { id: DatabaseProfile; name: string; description: string }[] = [
  { id: "byodb_clickhouse", name: "bring your own ClickHouse", description: "best for analytics and aggregations over large datasets." },
  { id: "byodb_postgres", name: "bring your own PostgreSQL", description: "best for transactional access and complex queries." },
  { id: "managed_rds", name: "AWS managed PostgreSQL (RDS)", description: "fully managed, adds ~$13-45/mo." },
  { id: "managed_clickhouse", name: "managed ClickHouse", description: "ClickHouse Cloud or equivalent." },
];

export function DatabaseStep({ state, dispatch }: Props) {
  return (
    <div className="space-y-5">
      {/* Qualifying question */}
      <div>
        <p className="text-[11px] uppercase tracking-[0.15em] text-[var(--color-text-muted)] mb-3">
          query patterns
        </p>
        <div className="flex gap-3">
          {(["analytics", "lookups"] as const).map((pattern) => (
            <button
              key={pattern}
              onClick={() => dispatch({ type: "SET_QUERY_PATTERN", pattern })}
              className={`
                flex-1 px-3 py-2.5 border text-[12px] transition-all
                ${state.queryPattern === pattern
                  ? "border-[var(--color-accent)]/40 text-[var(--color-accent)] bg-[var(--color-accent-dim)]"
                  : "border-[var(--color-border)] text-[var(--color-text-muted)] hover:border-[var(--color-border-hover)]"
                }
              `}
            >
              {pattern === "analytics" ? "analytics & aggregations" : "individual lookups"}
            </button>
          ))}
        </div>
        {state.queryPattern && (
          <p className="text-[11px] text-[var(--color-accent)] mt-2">
            → recommended: {state.queryPattern === "analytics" ? "ClickHouse" : "PostgreSQL"}
          </p>
        )}
      </div>

      {/* Database options */}
      <div className="space-y-2">
        {DB_OPTIONS.map((opt) => (
          <button
            key={opt.id}
            onClick={() => dispatch({ type: "SET_DATABASE_PROFILE", profile: opt.id })}
            className={`
              w-full text-left px-4 py-3 border transition-all
              ${state.databaseProfile === opt.id
                ? "border-[var(--color-accent)]/30 bg-[var(--color-accent-dim)]"
                : "border-[var(--color-border)] hover:border-[var(--color-border-hover)]"
              }
            `}
          >
            <span className={`text-[12px] ${state.databaseProfile === opt.id ? "text-[var(--color-accent)]" : "text-[var(--color-text)]"}`}>
              {opt.name}
            </span>
            <p className="text-[11px] text-[var(--color-text-muted)] mt-0.5">{opt.description}</p>
          </button>
        ))}
      </div>

      {/* Database name */}
      <div>
        <label className="block text-[11px] uppercase tracking-[0.15em] text-[var(--color-text-muted)] mb-2">
          database name
        </label>
        <input
          type="text"
          value={state.databaseName}
          onChange={(e) => dispatch({ type: "SET_DATABASE_NAME", name: e.target.value })}
          placeholder="rindexer"
          className="w-full px-3 py-2.5 bg-transparent border border-[var(--color-border)] text-[13px] text-[var(--color-text)] placeholder-[var(--color-text-muted)] focus:outline-none focus:border-[var(--color-accent)]/50 transition-colors"
        />
        <p className="text-[10px] text-[var(--color-text-muted)] mt-1">
          used in {state.databaseProfile.includes("clickhouse") ? "ClickHouse" : "PostgreSQL"} and rindexer.yaml storage config.
        </p>
      </div>
    </div>
  );
}
