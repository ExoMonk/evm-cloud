import type { Dispatch } from "react";
import type { BuilderState, BuilderAction } from "../../lib/configSchema.ts";
import { TEMPLATES } from "../../lib/templateRegistry.ts";
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
    </div>
  );
}
