import { useReducer, useState } from "react";
import { builderReducer, initialState } from "../../lib/configSchema.ts";
import { validate } from "../../lib/configValidator.ts";
import { TemplatePicker } from "./TemplatePicker.tsx";
import { StepForm } from "./StepForm.tsx";
import { PreviewPanel } from "./PreviewPanel.tsx";
import { FinalizeModal } from "./FinalizeModal.tsx";

export function BuilderPage() {
  const [state, dispatch] = useReducer(builderReducer, initialState);
  const [showFinalize, setShowFinalize] = useState(false);

  const errors = validate(state).filter((i) => i.severity === "error");

  return (
    <div className="max-w-6xl mx-auto px-5 md:px-16 py-12">
      {/* Hero */}
      <div className="mb-10">
        <h1 className="text-[clamp(28px,5vw,48px)] font-light text-[var(--color-text)] leading-tight">
          infrastructure builder
        </h1>
        <p className="text-[14px] text-[var(--color-text-dim)] mt-3 max-w-xl">
          Build your EVM data pipeline visually. Select components,
          preview configs, and export a ready-to-deploy project.
        </p>
      </div>

      {/* Template Picker */}
      <TemplatePicker state={state} dispatch={dispatch} />

      {/* Main: Step Form + Preview */}
      <div className="mt-10 flex flex-col lg:flex-row gap-8">
        {/* Left: Step Form (55%) */}
        <div className="lg:w-[53%]">
          <StepForm state={state} dispatch={dispatch} onFinalize={() => setShowFinalize(true)} />
        </div>

        {/* Right: Preview Panel (45%) */}
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
    </div>
  );
}
