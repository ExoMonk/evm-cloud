import { useState, useMemo } from "react";
import type { BuilderState } from "../../lib/configSchema.ts";
import { generateSchemaPreview, type TableDef, type MaterializedViewDef, type SampleQuery } from "../../lib/schemaPreview.ts";
import { CornerCard } from "../ui/CornerCard.tsx";

interface Props {
  state: BuilderState;
}

export function SchemaPreview({ state }: Props) {
  const schema = useMemo(() => generateSchemaPreview(state), [state]);

  if (!schema) {
    return (
      <div className="flex items-center justify-center py-16">
        <CornerCard className="p-8 text-center max-w-xs">
          <p className="text-[var(--color-accent)] text-[16px] mb-2">⬡</p>
          <p className="text-[12px] text-[var(--color-text-dim)]">no template selected</p>
          <p className="text-[11px] text-[var(--color-text-muted)] mt-1">
            pick a protocol template to see the database schema you'll get.
          </p>
        </CornerCard>
      </div>
    );
  }

  return (
    <div className="space-y-5 overflow-auto max-h-[70vh] scrollbar-none">
      {/* Database name */}
      <div className="flex items-center gap-2">
        <span className="text-[10px] text-[var(--color-text-muted)] uppercase tracking-[0.15em]">database</span>
        <span className="text-[12px] text-[var(--color-accent)]">{schema.databaseName}</span>
      </div>

      {/* Tables */}
      <div>
        <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-3">
          // tables
        </p>
        <div className="space-y-1.5">
          {schema.tables.map((table) => (
            <TableRow key={table.fullName} table={table} />
          ))}
        </div>
      </div>

      {/* Analytics Views */}
      {schema.materializedViews.length > 0 && (
        <div>
          <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-3">
            // analytics views
          </p>
          <div className="space-y-1.5">
            {schema.materializedViews.map((mv) => (
              <MVRow key={mv.name} mv={mv} />
            ))}
          </div>
          <p className="text-[10px] text-[var(--color-text-muted)] mt-2 px-3">
            analytics views compute automatically as new data arrives. no cron jobs needed.
          </p>
        </div>
      )}

      {/* Sample Queries */}
      {schema.sampleQueries.length > 0 && (
        <div>
          <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-3">
            // sample queries
          </p>
          <div className="space-y-3">
            {schema.sampleQueries.map((q, i) => (
              <QueryBlock key={i} query={q} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Table row — expandable
// ---------------------------------------------------------------------------

function TableRow({ table }: { table: TableDef }) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className={`border transition-colors ${expanded ? "border-[var(--color-accent)]/20" : "border-[var(--color-border)]"}`}>
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full px-3 py-2 flex items-center gap-2 text-left"
      >
        <span className="text-[10px] text-[var(--color-text-muted)]">
          {expanded ? "▾" : "▸"}
        </span>
        <span className="text-[12px] text-[var(--color-accent)] flex-1">{table.tableName}</span>
        <span className="text-[10px] text-[var(--color-text-muted)]">{table.columnCount} cols</span>
        <span className="text-[9px] px-1.5 py-0.5 border border-[var(--color-border)] text-[var(--color-text-muted)]">
          TABLE
        </span>
      </button>

      {expanded && (
        <div className="px-3 pb-3 border-t border-[var(--color-border)]">
          <p className="text-[10px] text-[var(--color-text-muted)] mt-2 mb-2">{table.description}</p>

          {/* Column list */}
          <div className="space-y-0.5">
            {table.columns.map((col) => (
              <div key={col.name} className="flex items-baseline gap-3 text-[11px] px-2 py-0.5">
                <span className={`w-36 truncate ${col.source === "event" ? "text-[var(--color-text)]" : "text-[var(--color-text-muted)]"}`}>
                  {col.name}
                </span>
                <span className="text-[var(--color-accent)] text-[10px]">{col.type}</span>
              </div>
            ))}
          </div>

          {/* ORDER BY */}
          <div className="mt-2 pt-2 border-t border-[var(--color-border)]">
            <span className="text-[10px] text-[var(--color-text-muted)]">
              ORDER BY: ({table.orderBy.join(", ")})
            </span>
          </div>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Materialized view row — expandable
// ---------------------------------------------------------------------------

function MVRow({ mv }: { mv: MaterializedViewDef }) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className={`border transition-colors ${expanded ? "border-blue-500/20" : "border-[var(--color-border)]"}`}>
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full px-3 py-2 flex items-center gap-2 text-left"
      >
        <span className="text-[10px] text-[var(--color-text-muted)]">
          {expanded ? "▾" : "▸"}
        </span>
        <span className="text-[12px] text-blue-400 flex-1">{mv.targetTable}</span>
        <span className="text-[9px] px-1.5 py-0.5 border border-blue-500/30 text-blue-400">
          VIEW
        </span>
      </button>

      {expanded && (
        <div className="px-3 pb-3 border-t border-[var(--color-border)]">
          <p className="text-[10px] text-[var(--color-text-dim)] mt-2">
            ◊ {mv.description}
          </p>
          <p className="text-[10px] text-[var(--color-text-muted)] mt-1">
            source: {mv.sourceTable} → {mv.aggregation}
          </p>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sample query block — copy-to-clipboard
// ---------------------------------------------------------------------------

function QueryBlock({ query }: { query: SampleQuery }) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    await navigator.clipboard.writeText(query.sql);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <div>
      <div className="flex items-center justify-between mb-1">
        <span className="text-[11px] text-[var(--color-text-dim)]">{query.title}</span>
        <button
          onClick={copy}
          className="text-[10px] text-[var(--color-text-muted)] hover:text-[var(--color-accent)] transition-colors"
        >
          {copied ? "copied" : "copy"}
        </button>
      </div>
      <pre className="p-3 text-[11px] leading-relaxed text-[var(--color-text-dim)] border border-[var(--color-border)] bg-[rgba(0,0,0,0.3)] overflow-x-auto scrollbar-none">
        {query.sql}
      </pre>
    </div>
  );
}
