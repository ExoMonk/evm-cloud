import { useState, useRef, type Dispatch } from "react";
import type { BuilderState, BuilderAction } from "../../lib/configSchema.ts";
import { initialState } from "../../lib/configSchema.ts";
import { EXAMPLES } from "../../lib/exampleConfigs.ts";
import { parseToml, tomlToBuilderState } from "../../lib/tomlParser.ts";
import { CornerCard } from "../ui/CornerCard.tsx";

interface Props {
  state: BuilderState;
  dispatch: Dispatch<BuilderAction>;
  onClose: () => void;
}

/**
 * Import modal — load config from examples or paste/upload TOML.
 */
export function ImportConfig({ state: _state, dispatch, onClose }: Props) {
  const [tab, setTab] = useState<"examples" | "import">("examples");
  const [tomlInput, setTomlInput] = useState("");
  const [error, setError] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const applyState = (partial: Partial<BuilderState>) => {
    // Reset to initial then apply overrides
    // We dispatch individual actions for each field
    const merged = { ...initialState, ...partial };

    if (merged.infraProfile) dispatch({ type: "SET_INFRA_PROFILE", profile: merged.infraProfile });
    dispatch({ type: "SET_PROJECT_NAME", name: merged.projectName });
    dispatch({ type: "SET_DATABASE_PROFILE", profile: merged.databaseProfile });
    dispatch({ type: "SET_DATABASE_NAME", name: merged.databaseName });
    dispatch({ type: "SET_CHAINS", chains: merged.chains });
    for (const [chain, url] of Object.entries(merged.rpcEndpoints)) {
      dispatch({ type: "SET_RPC_ENDPOINT", chain, url });
    }
    dispatch({ type: "SET_INGRESS_MODE", mode: merged.ingressMode });
    dispatch({ type: "SET_DOMAIN", domain: merged.domain });
    dispatch({ type: "SET_TLS_EMAIL", email: merged.tlsEmail });
    dispatch({ type: "SET_SECRETS_MODE", mode: merged.secretsMode });
    if (merged.monitoring) dispatch({ type: "SET_MONITORING", config: merged.monitoring });
    if (merged.streaming) dispatch({ type: "SET_STREAMING", config: merged.streaming });
    if (merged.networking) dispatch({ type: "SET_NETWORKING", config: merged.networking });
    if (merged.stateBackend) dispatch({ type: "SET_STATE_BACKEND", backend: merged.stateBackend });
    dispatch({ type: "SET_EXTRA_ENV", env: merged.extraEnv });
    if (merged.customServices.length > 0) {
      dispatch({ type: "SET_CUSTOM_SERVICES", services: merged.customServices });
    }

    onClose();
  };

  const loadExample = (example: typeof EXAMPLES[0]) => {
    applyState(example.state as Partial<BuilderState>);
  };

  const importToml = () => {
    try {
      setError(null);
      const parsed = parseToml(tomlInput);
      const partial = tomlToBuilderState(parsed);
      applyState(partial as Partial<BuilderState>);
    } catch (e) {
      setError(e instanceof Error ? e.message : "failed to parse TOML");
    }
  };

  const handleFileUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (ev) => {
      const content = ev.target?.result as string;
      setTomlInput(content);
    };
    reader.readAsText(file);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/80 backdrop-blur-sm" onClick={onClose} />

      <div className="relative w-[90vw] max-w-3xl max-h-[80vh] border border-[var(--color-border)] bg-[var(--color-bg)] flex flex-col">
        {/* Corners */}
        <div className="absolute top-0 left-0 w-3 h-3 border-t border-l border-[var(--color-accent)]" />
        <div className="absolute top-0 right-0 w-3 h-3 border-t border-r border-[var(--color-accent)]" />
        <div className="absolute bottom-0 left-0 w-3 h-3 border-b border-l border-[var(--color-accent)]" />
        <div className="absolute bottom-0 right-0 w-3 h-3 border-b border-r border-[var(--color-accent)]" />

        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-[var(--color-border)]">
          <div className="flex items-center gap-4">
            <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)]">// import</p>
            <div className="flex gap-3">
              <button
                onClick={() => setTab("examples")}
                className={`text-[11px] uppercase tracking-[0.15em] transition-colors ${
                  tab === "examples" ? "text-[var(--color-accent)]" : "text-[var(--color-text-muted)] hover:text-[var(--color-text)]"
                }`}
              >
                examples
              </button>
              <button
                onClick={() => setTab("import")}
                className={`text-[11px] uppercase tracking-[0.15em] transition-colors ${
                  tab === "import" ? "text-[var(--color-accent)]" : "text-[var(--color-text-muted)] hover:text-[var(--color-text)]"
                }`}
              >
                paste / upload
              </button>
            </div>
          </div>
          <button
            onClick={onClose}
            className="text-[11px] uppercase tracking-[0.15em] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
          >
            esc
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-6 scrollbar-none">
          {tab === "examples" ? (
            <div className="space-y-3">
              <p className="text-[11px] text-[var(--color-text-muted)] mb-4">
                pick a pre-built example to load its configuration into the builder.
              </p>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                {EXAMPLES.map((ex) => (
                  <CornerCard key={ex.name} hover className="cursor-pointer p-4" accent={false}>
                    <button onClick={() => loadExample(ex)} className="w-full text-left">
                      <div className="flex items-center justify-between">
                        <span className="text-[12px] text-[var(--color-text)]">{ex.displayName}</span>
                        <span className="text-[10px] text-[var(--color-accent)]">{ex.cost}</span>
                      </div>
                      <p className="text-[10px] text-[var(--color-text-muted)] mt-1">{ex.description}</p>
                      <p className="text-[9px] text-[var(--color-text-muted)]/60 mt-1">{ex.name}</p>
                    </button>
                  </CornerCard>
                ))}
              </div>
            </div>
          ) : (
            <div className="space-y-4">
              <p className="text-[11px] text-[var(--color-text-muted)]">
                paste an existing evm-cloud.toml or upload the file.
              </p>

              {/* Upload button */}
              <div className="flex items-center gap-3">
                <button
                  onClick={() => fileRef.current?.click()}
                  className="text-[11px] uppercase tracking-[0.15em] px-4 py-2 border border-[var(--color-border)] text-[var(--color-text-dim)] hover:border-[var(--color-accent)]/40 hover:text-[var(--color-accent)] transition-colors"
                >
                  upload file
                </button>
                <input
                  ref={fileRef}
                  type="file"
                  accept=".toml"
                  onChange={handleFileUpload}
                  className="hidden"
                />
                {tomlInput && (
                  <span className="text-[10px] text-[var(--color-accent)]">
                    {tomlInput.split("\n").length} lines loaded
                  </span>
                )}
              </div>

              {/* Paste area */}
              <textarea
                value={tomlInput}
                onChange={(e) => setTomlInput(e.target.value)}
                placeholder={'# Paste your evm-cloud.toml here\n\nschema_version = 1\n\n[project]\nname = "my-project"\nregion = "us-east-1"\n...'}
                className="w-full h-48 px-4 py-3 bg-transparent border border-[var(--color-border)] text-[12px] text-[var(--color-text)] placeholder-[var(--color-text-muted)]/30 focus:outline-none focus:border-[var(--color-accent)]/50 transition-colors resize-none scrollbar-none"
              />

              {error && (
                <p className="text-[11px] text-[var(--color-error)]">✕ {error}</p>
              )}

              <button
                disabled={!tomlInput.trim()}
                onClick={importToml}
                className={`
                  w-full py-3 text-[11px] uppercase tracking-[0.2em] font-medium border transition-all
                  ${!tomlInput.trim()
                    ? "border-[var(--color-border)] text-[var(--color-text-muted)] cursor-not-allowed"
                    : "border-[var(--color-accent)] text-[var(--color-accent)] hover:bg-[var(--color-accent-dim)]"
                  }
                `}
              >
                import configuration
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
