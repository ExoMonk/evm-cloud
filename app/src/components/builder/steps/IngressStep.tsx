import type { Dispatch } from "react";
import type { BuilderState, BuilderAction, IngressMode } from "../../../lib/configSchema.ts";
import { INGRESS_MODES_BY_ENGINE } from "../../../lib/configSchema.ts";

interface Props {
  state: BuilderState;
  dispatch: Dispatch<BuilderAction>;
}

const INGRESS_DESCRIPTIONS: Record<IngressMode, string> = {
  none: "internal only — no public endpoint.",
  cloudflare: "Cloudflare DNS with zero-config TLS.",
  caddy: "automatic Let's Encrypt certificates.",
  ingress_nginx: "standard K8s ingress controller + cert-manager.",
};

export function IngressStep({ state, dispatch }: Props) {
  const validModes = INGRESS_MODES_BY_ENGINE[state.computeEngine];
  const hasIngress = state.ingressMode !== "none";
  const needsTls = state.ingressMode === "caddy" || state.ingressMode === "ingress_nginx";

  return (
    <div className="space-y-5">
      {/* Ingress mode selection */}
      <div className="space-y-2">
        {validModes.map((mode) => {
          const isSelected = state.ingressMode === mode;
          return (
            <button
              key={mode}
              onClick={() => dispatch({ type: "SET_INGRESS_MODE", mode })}
              className={`
                w-full text-left px-4 py-3 border transition-all
                ${isSelected
                  ? "border-[var(--color-accent)]/30 bg-[var(--color-accent-dim)]"
                  : "border-[var(--color-border)] hover:border-[var(--color-border-hover)]"
                }
              `}
            >
              <span className={`text-[12px] ${isSelected ? "text-[var(--color-accent)]" : "text-[var(--color-text)]"}`}>
                {mode}
              </span>
              <p className="text-[11px] text-[var(--color-text-muted)] mt-0.5">
                {INGRESS_DESCRIPTIONS[mode]}
              </p>
            </button>
          );
        })}
      </div>

      {/* eRPC hostname — only when ingress is enabled */}
      {hasIngress && (
        <div>
          <label className="block text-[11px] uppercase tracking-[0.15em] text-[var(--color-text-muted)] mb-2">
            erpc hostname
          </label>
          <input
            type="text"
            value={state.domain}
            onChange={(e) => dispatch({ type: "SET_DOMAIN", domain: e.target.value })}
            placeholder="rpc.example.com"
            className="w-full px-3 py-2.5 bg-transparent border border-[var(--color-border)] text-[13px] text-[var(--color-text)] placeholder-[var(--color-text-muted)] focus:outline-none focus:border-[var(--color-accent)]/50 transition-colors"
          />
          <p className="text-[10px] text-[var(--color-text-muted)] mt-1">
            public hostname for your eRPC proxy endpoint.
          </p>
        </div>
      )}

      {/* TLS email — only for Let's Encrypt modes */}
      {needsTls && (
        <div>
          <label className="block text-[11px] uppercase tracking-[0.15em] text-[var(--color-text-muted)] mb-2">
            tls email (let's encrypt)
          </label>
          <input
            type="email"
            value={state.tlsEmail}
            onChange={(e) => dispatch({ type: "SET_TLS_EMAIL", email: e.target.value })}
            placeholder="admin@example.com"
            className="w-full px-3 py-2.5 bg-transparent border border-[var(--color-border)] text-[13px] text-[var(--color-text)] placeholder-[var(--color-text-muted)] focus:outline-none focus:border-[var(--color-accent)]/50 transition-colors"
          />
        </div>
      )}
    </div>
  );
}
