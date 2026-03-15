import type { Dispatch } from "react";
import type { BuilderState, BuilderAction } from "../../lib/configSchema.ts";
import { validate } from "../../lib/configValidator.ts";
import { SectionHeader } from "../ui/SectionHeader.tsx";
import { ProjectStep } from "./steps/ProjectStep.tsx";
import { InfraProfileStep } from "./steps/InfraProfileStep.tsx";
import { DatabaseStep } from "./steps/DatabaseStep.tsx";
import { ChainsRpcStep } from "./steps/ChainsRpcStep.tsx";
import { IngressStep } from "./steps/IngressStep.tsx";
import { ReviewStep } from "./steps/ReviewStep.tsx";

interface Props {
  state: BuilderState;
  dispatch: Dispatch<BuilderAction>;
  onFinalize: () => void;
}

const STEPS = [
  { label: "project", component: ProjectStep },
  { label: "infrastructure", component: InfraProfileStep },
  { label: "database", component: DatabaseStep },
  { label: "chains & rpc", component: ChainsRpcStep },
  { label: "ingress", component: IngressStep },
  { label: "review & export", component: ReviewStep },
] as const;

export function StepForm({ state, dispatch, onFinalize }: Props) {
  const errors = validate(state).filter((i) => i.severity === "error");

  const setStep = (step: number) => {
    dispatch({ type: "SET_STEP", step });
  };

  return (
    <div>
      <SectionHeader label="configure" />
      <div className="space-y-3">
        {STEPS.map((step, index) => {
          const isActive = state.currentStep === index;
          const isCompleted = state.completedSteps.has(index);
          const StepComponent = step.component;

          return (
            <div
              key={step.label}
              className={`
                border transition-all duration-250
                ${isActive
                  ? "border-[var(--color-accent)]/30 bg-[var(--color-surface)]"
                  : isCompleted
                    ? "border-[var(--color-border)] bg-[var(--color-surface)]"
                    : "border-[var(--color-border)] bg-transparent"
                }
              `}
            >
              {/* Step header */}
              <button
                onClick={() => setStep(index)}
                className="w-full px-5 py-3.5 flex items-center gap-3 text-left"
              >
                <span
                  className={`
                    text-[11px] font-medium w-5 text-center
                    ${isActive
                      ? "text-[var(--color-accent)]"
                      : isCompleted
                        ? "text-[var(--color-accent)]"
                        : "text-[var(--color-text-muted)]"
                    }
                  `}
                >
                  {isCompleted ? "✓" : `${index + 1}`}
                </span>
                <span
                  className={`
                    text-[12px] uppercase tracking-[0.15em]
                    ${isActive
                      ? "text-[var(--color-text)]"
                      : isCompleted
                        ? "text-[var(--color-text-dim)]"
                        : "text-[var(--color-text-muted)]"
                    }
                  `}
                >
                  {step.label}
                </span>
                {isActive && (
                  <span className="ml-auto w-1.5 h-1.5 bg-[var(--color-accent)] rounded-full" />
                )}
              </button>

              {/* Step content */}
              {isActive && (
                <div className="px-5 pb-5 border-t border-[var(--color-border)]">
                  <div className="pt-4">
                    <StepComponent state={state} dispatch={dispatch} />
                  </div>
                  <div className="flex justify-between mt-5">
                    {index > 0 && (
                      <button
                        onClick={() => setStep(index - 1)}
                        className="text-[11px] uppercase tracking-[0.15em] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
                      >
                        ← back
                      </button>
                    )}
                    {index < STEPS.length - 1 && (
                      <button
                        onClick={() => {
                          dispatch({ type: "COMPLETE_STEP", step: index });
                          setStep(index + 1);
                        }}
                        className="ml-auto text-[11px] uppercase tracking-[0.15em] px-4 py-1.5 border border-[var(--color-accent)] text-[var(--color-accent)] hover:bg-[var(--color-accent-dim)] transition-colors"
                      >
                        next →
                      </button>
                    )}
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* Finalize button — always visible at bottom of configure */}
      <button
        disabled={errors.length > 0}
        onClick={onFinalize}
        className={`
          w-full mt-5 py-3.5 text-[12px] uppercase tracking-[0.2em] font-medium border transition-all
          ${errors.length > 0
            ? "border-[var(--color-border)] text-[var(--color-text-muted)] cursor-not-allowed"
            : "border-[var(--color-accent)] text-[var(--color-accent)] hover:bg-[var(--color-accent)] hover:text-[var(--color-bg)]"
          }
        `}
      >
        // finalize & export
      </button>
    </div>
  );
}
