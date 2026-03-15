import type { Dispatch } from "react";
import type { BuilderState, BuilderAction } from "../../../lib/configSchema.ts";
import { AVAILABLE_CHAINS } from "../../../lib/configSchema.ts";

interface Props {
  state: BuilderState;
  dispatch: Dispatch<BuilderAction>;
}

export function ChainsRpcStep({ state, dispatch }: Props) {
  const toggleChain = (chain: string) => {
    const chains = state.chains.includes(chain)
      ? state.chains.filter((c) => c !== chain)
      : [...state.chains, chain];
    dispatch({ type: "SET_CHAINS", chains });
  };

  return (
    <div className="space-y-5">
      {/* Chain selection */}
      <div>
        <p className="text-[11px] uppercase tracking-[0.15em] text-[var(--color-text-muted)] mb-3">
          chains
        </p>
        <div className="grid grid-cols-2 sm:grid-cols-3 gap-2">
          {AVAILABLE_CHAINS.map((chain) => {
            const isSelected = state.chains.includes(chain);
            return (
              <button
                key={chain}
                onClick={() => toggleChain(chain)}
                className={`
                  px-3 py-2 border text-[12px] transition-all
                  ${isSelected
                    ? "border-[var(--color-accent)]/40 text-[var(--color-accent)] bg-[var(--color-accent-dim)]"
                    : "border-[var(--color-border)] text-[var(--color-text-muted)] hover:border-[var(--color-border-hover)]"
                  }
                `}
              >
                <span className="flex items-center gap-2">
                  <span className={`w-1.5 h-1.5 rounded-full ${isSelected ? "bg-[var(--color-accent)]" : "bg-[var(--color-text-muted)]/30"}`} />
                  {chain}
                </span>
              </button>
            );
          })}
        </div>
        {state.chains.length === 0 && (
          <p className="text-[11px] text-[var(--color-error)] mt-1.5">select at least one chain.</p>
        )}
      </div>

      {/* RPC endpoints */}
      {state.chains.length > 0 && (
        <div className="space-y-3">
          <p className="text-[11px] uppercase tracking-[0.15em] text-[var(--color-text-muted)]">
            rpc endpoints
          </p>
          {state.chains.map((chain) => (
            <div key={chain}>
              <label className="block text-[10px] text-[var(--color-text-muted)] mb-1">{chain}</label>
              <input
                type="text"
                value={state.rpcEndpoints[chain] ?? ""}
                onChange={(e) =>
                  dispatch({ type: "SET_RPC_ENDPOINT", chain, url: e.target.value })
                }
                placeholder={`https://${chain}-mainnet.g.alchemy.com/v2/YOUR_KEY`}
                className="w-full px-3 py-2 bg-transparent border border-[var(--color-border)] text-[12px] text-[var(--color-text)] placeholder-[var(--color-text-muted)]/50 focus:outline-none focus:border-[var(--color-accent)]/50 transition-colors"
              />
            </div>
          ))}
          <p className="text-[10px] text-[var(--color-text-muted)]">
            public RPCs are rate-limited — fine for testing.
          </p>
        </div>
      )}
    </div>
  );
}
