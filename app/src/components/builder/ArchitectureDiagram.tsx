import type { BuilderState } from "../../lib/configSchema.ts";
import { getChainId } from "../../lib/chains.ts";

interface Props {
  state: BuilderState;
  compact?: boolean;
}

/**
 * System design diagram with connection arrows showing data flow.
 * Nested boxes: Provider > VPC > Compute > Services
 * Arrows: chains → eRPC → rindexer → DB/streaming
 */
export function ArchitectureDiagram({ state, compact = false }: Props) {
  const isAws = state.provider === "aws";
  const isK8s = state.computeEngine === "k3s" || state.computeEngine === "eks";
  const isClickhouse = state.databaseProfile.includes("clickhouse");
  const dbLabel = isClickhouse ? "ClickHouse" : "PostgreSQL";
  const dbExternal = state.databaseProfile.startsWith("byodb");
  const hasMonitoring = state.monitoring?.enabled;
  const hasStreaming = state.streaming && state.streaming.mode !== "disabled";
  const streamLabel = state.streaming?.mode === "kafka" ? "Kafka"
    : state.streaming?.mode === "sns-sqs" ? "SNS/SQS"
    : state.streaming?.mode === "cdc" ? "CDC" : null;
  const hasEso = state.secretsMode === "external" && isK8s;
  const hasSecretsManager = state.secretsMode === "provider" && isAws;

  const engineLabel: Record<string, string> = {
    ec2: "EC2", eks: "EKS", k3s: "k3s", docker_compose: "Docker Compose",
  };
  const ingressLabel: Record<string, string> = {
    cloudflare: "Cloudflare", caddy: "Caddy", ingress_nginx: "nginx + cert-manager",
  };

  const s = compact
    ? { box: "p-3", inner: "p-2.5", pt: "pt-5", gap: "gap-2", mt: "mt-3", txt: "text-[9px]", txs: "text-[8px]", nd: "px-2 py-1" }
    : { box: "p-5", inner: "p-4", pt: "pt-7", gap: "gap-3", mt: "mt-4", txt: "text-[10px]", txs: "text-[9px]", nd: "px-3 py-1.5" };

  return (
    <div className={`space-y-4 ${compact ? "" : "space-y-5"}`}>

      {/* ═══ Provider box ═══ */}
      <div className="border border-[var(--color-border)] relative">
        <Lbl accent className={s.txt}>{isAws ? `AWS ${state.region}` : "bare metal"}</Lbl>

        <div className={`${s.box} ${s.pt}`}>

          {/* VPC (AWS only) */}
          {isAws && (
            <div className={`border border-[var(--color-border)] border-dashed relative ${s.mt}`}>
              <Lbl className={s.txs}>VPC {state.networking?.vpcCidr ?? "10.42.0.0/16"}</Lbl>

              <div className={`${s.inner} ${s.pt}`}>
                <ComputeInner
                  label={`${engineLabel[state.computeEngine]} ${state.instanceType ? `(${state.instanceType})` : ""}`}
                  s={s} state={state} isK8s={isK8s} hasMonitoring={hasMonitoring} hasEso={hasEso} ingressLabel={ingressLabel}
                />
              </div>
            </div>
          )}

          {/* Bare metal: no VPC wrapper */}
          {!isAws && (
            <ComputeInner
              label={engineLabel[state.computeEngine]}
              s={s} state={state} isK8s={isK8s} hasMonitoring={hasMonitoring} hasEso={hasEso} ingressLabel={ingressLabel}
            />
          )}

          {/* ── Connection: rindexer outputs ── */}
          <div className={`flex items-start ${s.gap} ${s.mt}`}>
            <div className={`${s.txs} text-[var(--color-text-muted)] w-16 shrink-0 pt-1`}>outputs</div>
            <div className="flex-1">
              {/* Arrow from compute to outputs */}
              <div className="flex items-center gap-1 mb-2">
                <div className="w-0 h-0 border-t-[4px] border-t-transparent border-b-[4px] border-b-transparent border-l-[5px] border-l-[var(--color-accent)]/40 rotate-90" />
                <span className={`${s.txs} text-[var(--color-accent)]/40`}>rindexer writes to</span>
              </div>
              <div className={`flex ${s.gap} flex-wrap`}>
                <Nd label={dbLabel} sub={dbExternal ? "BYO external" : "managed"} s={s} />
                {hasStreaming && streamLabel && <Nd label={streamLabel} sub="streaming" s={s} />}
              </div>
            </div>
          </div>

          {/* ── External services ── */}
          {hasSecretsManager && (
            <div className={`flex items-center ${s.gap} ${s.mt}`}>
              <div className={`${s.txs} text-[var(--color-text-muted)] w-16 shrink-0`}>services</div>
              <Nd label="Secrets Manager" sub="AWS" s={s} />
            </div>
          )}
        </div>
      </div>

      {/* ═══ Chains (external input) ═══ */}
      <div className={`flex items-center ${s.gap}`}>
        <div className={`${s.txs} text-[var(--color-text-muted)] uppercase tracking-[0.15em] w-16 shrink-0`}>chains</div>
        <div className="flex items-center gap-1">
          {/* Arrow into system */}
          <div className={`flex flex-col ${s.gap}`}>
            {state.chains.length > 0 ? (
              state.chains.map((chain) => (
                <span key={chain} className={`${s.nd} border border-[var(--color-accent)]/30 bg-[var(--color-accent-dim)] ${s.txt} text-[var(--color-accent)]`}>
                  {chain} ({getChainId(chain)})
                </span>
              ))
            ) : (
              <span className={`${s.nd} border border-dashed border-[var(--color-border)] ${s.txt} text-[var(--color-text-muted)] opacity-40`}>
                no chains selected
              </span>
            )}
          </div>
          {state.chains.length > 0 && (
            <div className="flex items-center px-1">
              <div className="w-6 h-px bg-[var(--color-accent)]/30" />
              <div className="w-0 h-0 border-t-[3px] border-t-transparent border-b-[3px] border-b-transparent border-l-[5px] border-l-[var(--color-accent)]/30" />
              <span className={`${s.txs} text-[var(--color-accent)]/40 ml-1`}>→ eRPC</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Compute inner content (shared between AWS/VPC and bare metal paths)
// ---------------------------------------------------------------------------

function ComputeInner({ label, s, state, isK8s, hasMonitoring, hasEso, ingressLabel }: {
  label: string;
  s: Record<string, string>;
  state: BuilderState;
  isK8s: boolean;
  hasMonitoring?: boolean;
  hasEso: boolean;
  ingressLabel: Record<string, string>;
}) {
  return (
    <div className={`border border-[var(--color-accent)]/20 bg-[var(--color-accent-dim)] relative ${s.mt}`}>
      <Lbl accent className={s.txt}>{label}</Lbl>

      <div className={`${s.inner} ${s.pt}`}>
        {/* Data pipeline row */}
        <div className={`flex items-center ${s.gap} flex-wrap`}>
          <Nd label="eRPC" sub="rpc proxy" s={s} />
          <Arr s={s} />
          <Nd label="rindexer" sub="indexer" s={s} />
        </div>

        {/* Services row */}
        {(state.ingressMode !== "none" || hasMonitoring || hasEso) && (
          <div className={`flex items-center ${s.gap} flex-wrap ${s.mt}`}>
            {state.ingressMode !== "none" && (
              <Nd label={ingressLabel[state.ingressMode] ?? state.ingressMode} sub={isK8s ? "ingress" : "TLS"} s={s} />
            )}
            {hasMonitoring && <Nd label="Grafana" sub="monitoring" s={s} />}
            {hasEso && <Nd label="ESO" sub="secrets" s={s} />}
          </div>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

function Lbl({ children, className, accent }: { children: React.ReactNode; className: string; accent?: boolean }) {
  return (
    <div className="absolute -top-2.5 left-3 px-1.5 bg-[var(--color-bg)]">
      <span className={`${className} uppercase tracking-[0.15em] ${accent ? "text-[var(--color-accent)]" : "text-[var(--color-text-muted)]"}`}>
        {children}
      </span>
    </div>
  );
}

function Nd({ label, sub, s }: { label: string; sub: string; s: Record<string, string> }) {
  return (
    <div className={`${s.nd} border border-[var(--color-border)] bg-[var(--color-surface)]`}>
      <span className={`${s.txt} text-[var(--color-text)] block`}>{label}</span>
      <span className={`${s.txs} text-[var(--color-text-muted)] block`}>{sub}</span>
    </div>
  );
}

function Arr({ s: _s }: { s: Record<string, string> }) {
  return (
    <div className="flex items-center shrink-0">
      <div className="w-4 h-px bg-[var(--color-accent)]/40" />
      <div className="w-0 h-0 border-t-[3px] border-t-transparent border-b-[3px] border-b-transparent border-l-[5px] border-l-[var(--color-accent)]/40" />
    </div>
  );
}
