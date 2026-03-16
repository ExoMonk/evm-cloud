import { useReducer, useState } from "react";
import { builderReducer, initialState } from "../../lib/configSchema.ts";
import { validate } from "../../lib/configValidator.ts";
import { TemplatePicker } from "./TemplatePicker.tsx";
import { StepForm } from "./StepForm.tsx";
import { PreviewPanel } from "./PreviewPanel.tsx";
import { FinalizeModal } from "./FinalizeModal.tsx";
import { ImportConfig } from "./ImportConfig.tsx";
import { ArchitectureDiagram } from "./ArchitectureDiagram.tsx";

export function BuilderPage() {
  const [state, dispatch] = useReducer(builderReducer, initialState);
  const [showFinalize, setShowFinalize] = useState(false);
  const [showImport, setShowImport] = useState(false);

  const errors = validate(state).filter((i) => i.severity === "error");

  return (
    <div className="max-w-6xl mx-auto px-5 md:px-16 py-12">
      {/* Hero: title + import button + live architecture diagram */}
      <div className="mb-8">
        <div className="flex items-center justify-between mb-4">
          <h1 className="text-[14px] uppercase tracking-[0.2em] text-[var(--color-text-muted)]">
            // infrastructure builder
          </h1>
          <button
            onClick={() => setShowImport(true)}
            className="text-[11px] uppercase tracking-[0.15em] px-4 py-1.5 border border-[var(--color-border)] text-[var(--color-text-muted)] hover:border-[var(--color-accent)]/40 hover:text-[var(--color-accent)] transition-colors"
          >
            import / examples
          </button>
        </div>
        <div className="border border-[var(--color-border)] bg-[var(--color-surface)] p-4 relative">
          {/* Corner decorations */}
          <div className="absolute top-0 left-0 w-2.5 h-2.5 border-t border-l border-[var(--color-accent)]" />
          <div className="absolute top-0 right-0 w-2.5 h-2.5 border-t border-r border-[var(--color-accent)]" />
          <div className="absolute bottom-0 left-0 w-2.5 h-2.5 border-b border-l border-[var(--color-accent)]" />
          <div className="absolute bottom-0 right-0 w-2.5 h-2.5 border-b border-r border-[var(--color-accent)]" />

          <ArchitectureDiagram state={state} compact />
        </div>
      </div>

      {/* Template Picker */}
      <TemplatePicker state={state} dispatch={dispatch} />

      {/* Main: Step Form + Preview */}
      <div className="mt-10 flex flex-col lg:flex-row gap-8">
        {/* Left: Step Form (53%) */}
        <div className="lg:w-[53%]">
          <StepForm state={state} dispatch={dispatch} onFinalize={() => setShowFinalize(true)} />
        </div>

        {/* Right: Preview Panel (47%) */}
        <div className="lg:w-[47%]">
          <div className="sticky top-20">
            <PreviewPanel state={state} />
          </div>
        </div>
      </div>

      {/* Floating finalize trigger */}
      <button
        onClick={() => setShowFinalize(true)}
        className="fixed bottom-6 right-6 z-40 flex items-center gap-2 px-5 py-3 border border-[var(--color-accent)]/50 bg-[var(--color-bg)]/90 backdrop-blur-sm hover:bg-[var(--color-accent-dim)] hover:border-[var(--color-accent)] transition-all group"
      >
        <span className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] group-hover:text-[var(--color-accent)] transition-colors">
          // finalize
        </span>
        {errors.length === 0 ? (
          <span className="w-1.5 h-1.5 bg-[var(--color-accent)] rounded-full" />
        ) : (
          <span className="text-[10px] text-[var(--color-error)]">{errors.length}</span>
        )}
      </button>

      {/* Finalize modal */}
      {showFinalize && (
        <FinalizeModal
          state={state}
          onClose={() => setShowFinalize(false)}
        />
      )}

      {/* Import modal */}
      {showImport && (
        <ImportConfig
          state={state}
          dispatch={dispatch}
          onClose={() => setShowImport(false)}
        />
      )}
    </div>
  );
}
