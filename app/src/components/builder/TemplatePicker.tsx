import type { Dispatch } from "react";
import type { BuilderState, BuilderAction } from "../../lib/configSchema.ts";
import { TEMPLATES, type TemplateDef } from "../../lib/templateRegistry.ts";
import { CornerCard } from "../ui/CornerCard.tsx";
import { SectionHeader } from "../ui/SectionHeader.tsx";

const CATEGORY_COLORS: Record<string, string> = {
  token: "text-blue-400 border-blue-400/40",
  nft: "text-purple-400 border-purple-400/40",
  dex: "text-[var(--color-accent)] border-[var(--color-accent)]/40",
  lending: "text-amber-400 border-amber-400/40",
};

interface Props {
  state: BuilderState;
  dispatch: Dispatch<BuilderAction>;
}

export function TemplatePicker({ state, dispatch }: Props) {
  const selectTemplate = (templateName: string | null) => {
    if (templateName === null) {
      dispatch({ type: "SELECT_TEMPLATE", template: null, chains: [] });
      return;
    }
    const tmpl = TEMPLATES.find((t) => t.name === templateName);
    if (!tmpl) return;

    dispatch({
      type: "SELECT_TEMPLATE",
      template: templateName,
      chains: tmpl.chains.map((c) => c.chain),
      variables: Object.fromEntries(
        Object.entries(tmpl.variables ?? {}).map(([k, v]) => [k, v.default ?? ""])
      ),
    });
  };

  const selectedTmpl = TEMPLATES.find((t) => t.name === state.selectedTemplate);

  return (
    <div>
      <SectionHeader label="template" />
      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-4">
        {TEMPLATES.map((tmpl) => {
          const isSelected = state.selectedTemplate === tmpl.name;
          return (
            <CornerCard
              key={tmpl.name}
              accent={isSelected}
              hover
              className={`cursor-pointer p-4 ${isSelected ? "bg-[var(--color-accent-dim)]" : ""}`}
            >
              <button
                onClick={() => selectTemplate(tmpl.name)}
                className="w-full text-left"
              >
                <span className={`text-[10px] uppercase tracking-[0.15em] px-1.5 py-0.5 border ${CATEGORY_COLORS[tmpl.category]}`}>
                  {tmpl.category}
                </span>
                <p className={`text-[12px] font-medium mt-2.5 ${isSelected ? "text-[var(--color-accent)]" : "text-[var(--color-text)]"}`}>
                  {tmpl.displayName}
                </p>
                <p className="text-[11px] text-[var(--color-text-muted)] mt-1">
                  {tmpl.chains.length} chain{tmpl.chains.length !== 1 ? "s" : ""}
                </p>
              </button>
            </CornerCard>
          );
        })}

        {/* Blank */}
        <CornerCard
          accent={state.selectedTemplate === null}
          hover
          className={`cursor-pointer p-4 ${state.selectedTemplate === null ? "bg-[var(--color-accent-dim)]" : ""}`}
        >
          <button
            onClick={() => selectTemplate(null)}
            className="w-full text-left"
          >
            <p className="text-[12px] font-medium text-[var(--color-text-dim)] mt-2.5">blank</p>
            <p className="text-[11px] text-[var(--color-text-muted)] mt-1">start from scratch</p>
          </button>
        </CornerCard>
      </div>

      {/* Template details panel — appears when a template is selected */}
      {selectedTmpl && (
        <TemplateDetails
          template={selectedTmpl}
          state={state}
          dispatch={dispatch}
        />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------

function TemplateDetails({
  template,
  state,
  dispatch,
}: {
  template: TemplateDef;
  state: BuilderState;
  dispatch: Dispatch<BuilderAction>;
}) {
  const hasVariables = template.variables && Object.keys(template.variables).length > 0;
  const hasContracts = template.chains.some((c) =>
    Object.keys(c.contracts).length > 0
  );

  return (
    <CornerCard accent className="mt-4 p-5">
      <div className="flex flex-col lg:flex-row gap-6">

        {/* Left: events + contracts */}
        <div className="flex-1 space-y-4">
          {/* Events */}
          <div>
            <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-2">
              // events indexed
            </p>
            <div className="space-y-1">
              {template.events.map((event, i) => (
                <p key={i} className="text-[11px] text-[var(--color-text-dim)] break-all">
                  <span className="text-[var(--color-accent)]">→</span> {event}
                </p>
              ))}
            </div>
          </div>

          {/* Contracts per chain */}
          {hasContracts && (
            <div>
              <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-2">
                // contracts
              </p>
              <div className="space-y-2">
                {template.chains.map((chain) => {
                  const contracts = Object.entries(chain.contracts);
                  if (contracts.length === 0) return null;
                  return (
                    <div key={chain.chain}>
                      <p className="text-[11px] text-[var(--color-text-dim)]">{chain.chain}</p>
                      {contracts.map(([name, info]) => (
                        <div key={name} className="ml-3 mt-0.5">
                          <p className="text-[10px] text-[var(--color-text-muted)]">
                            {name}: <span className="text-[var(--color-text-dim)]">{truncateAddress(info.address)}</span>
                          </p>
                          <p className="text-[10px] text-[var(--color-text-muted)]">
                            start: <span className="text-[var(--color-text-dim)]">{info.startBlock.toLocaleString()}</span>
                          </p>
                        </div>
                      ))}
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </div>

        {/* Right: template variables (user inputs) */}
        {hasVariables && (
          <div className="lg:w-64 lg:border-l lg:border-[var(--color-border)] lg:pl-6 space-y-3">
            <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-2">
              // variables
            </p>
            {Object.entries(template.variables!).map(([key, def]) => (
              <div key={key}>
                <label className="block text-[10px] text-[var(--color-text-muted)] mb-1">
                  {key}
                </label>
                <input
                  type="text"
                  value={state.templateVariables[key] ?? def.default ?? ""}
                  onChange={(e) =>
                    dispatch({ type: "SET_TEMPLATE_VARIABLE", key, value: e.target.value })
                  }
                  placeholder={def.description}
                  className="w-full px-3 py-2 bg-transparent border border-[var(--color-border)] text-[12px] text-[var(--color-text)] placeholder-[var(--color-text-muted)]/50 focus:outline-none focus:border-[var(--color-accent)]/50 transition-colors"
                />
                <p className="text-[10px] text-[var(--color-text-muted)] mt-0.5">
                  {def.description}
                </p>
              </div>
            ))}
          </div>
        )}
      </div>
    </CornerCard>
  );
}

function truncateAddress(addr: string): string {
  if (addr.length <= 14) return addr;
  return `${addr.slice(0, 8)}...${addr.slice(-6)}`;
}
