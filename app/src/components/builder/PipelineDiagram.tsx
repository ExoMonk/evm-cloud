import { useMemo } from "react";
import type { BuilderState } from "../../lib/configSchema.ts";
import { getChainId } from "../../lib/chains.ts";

interface Props {
  state: BuilderState;
}

/**
 * Reactive pipeline diagram — shows the full architecture being built.
 * Two rows: infra layer (top) + data pipeline (bottom).
 */
export function PipelineDiagram({ state }: Props) {
  const infra = useMemo(() => buildInfraNodes(state), [state]);

  return (
    <div className="space-y-3">
      {/* Top row: infrastructure layer */}
      <div className="flex items-center gap-2 flex-wrap">
        <span className="text-[9px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] w-12 shrink-0">infra</span>
        {infra.map((node) => (
          <Tag key={node.id} label={node.label} active={node.active} accent={node.accent} />
        ))}
      </div>

      {/* Bottom row: data pipeline */}
      <div className="flex items-center gap-0 overflow-x-auto scrollbar-none">
        <span className="text-[9px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] w-12 shrink-0">data</span>

        {/* Chains */}
        <div className="flex flex-col gap-0.5 shrink-0">
          {state.chains.length > 0 ? (
            state.chains.map((chain) => (
              <PipeNode key={chain} label={`${chain} (${getChainId(chain)})`} active />
            ))
          ) : (
            <PipeNode label="chain" active={false} />
          )}
        </div>

        <Arrow />
        <PipeNode label="eRPC" active={state.chains.length > 0} />
        <Arrow />
        <PipeNode label="rindexer" active={state.chains.length > 0} />
        <Arrow />

        {/* Outputs: database + streaming (consumers of rindexer) */}
        <div className="flex flex-col gap-0.5 shrink-0">
          <PipeNode
            label={state.databaseProfile.includes("clickhouse") ? "ClickHouse" : "PostgreSQL"}
            active
          />
          {state.streaming && state.streaming.mode !== "disabled" && (
            <PipeNode
              label={state.streaming.mode === "kafka" ? "Kafka" : state.streaming.mode === "sns-sqs" ? "SNS/SQS" : "CDC"}
              active
            />
          )}
        </div>

        {/* Monitoring (observes everything) */}
        {state.monitoring?.enabled && (
          <>
            <Arrow />
            <PipeNode label="Grafana" active />
          </>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Infra nodes — shows the infrastructure layer
// ---------------------------------------------------------------------------

interface InfraNode {
  id: string;
  label: string;
  active: boolean;
  accent?: boolean;
}

function buildInfraNodes(state: BuilderState): InfraNode[] {
  const nodes: InfraNode[] = [];

  // Provider
  nodes.push({
    id: "provider",
    label: state.provider === "aws" ? "AWS" : "bare metal",
    active: true,
    accent: true,
  });

  // Compute engine
  const engineLabels: Record<string, string> = {
    ec2: "EC2",
    eks: "EKS",
    k3s: "k3s",
    docker_compose: "Docker Compose",
  };
  nodes.push({
    id: "engine",
    label: engineLabels[state.computeEngine] ?? state.computeEngine,
    active: !!state.infraProfile,
    accent: true,
  });

  // VPC (AWS only)
  if (state.provider === "aws") {
    nodes.push({
      id: "vpc",
      label: `VPC ${state.networking?.vpcCidr ?? "10.42.0.0/16"}`,
      active: true,
    });
  }

  // Region
  if (state.provider === "aws" && state.region) {
    nodes.push({
      id: "region",
      label: state.region,
      active: true,
    });
  }

  // Instance type
  if (state.instanceType) {
    nodes.push({
      id: "instance",
      label: state.instanceType,
      active: true,
    });
  }

  // Ingress
  if (state.ingressMode !== "none") {
    const ingressLabels: Record<string, string> = {
      cloudflare: "Cloudflare",
      caddy: "Caddy TLS",
      ingress_nginx: "nginx + cert-manager",
    };
    nodes.push({
      id: "ingress",
      label: ingressLabels[state.ingressMode] ?? state.ingressMode,
      active: true,
    });
  }

  // Secrets
  if (state.secretsMode !== "inline") {
    nodes.push({
      id: "secrets",
      label: state.secretsMode === "provider" ? "Secrets Manager" : "ESO",
      active: true,
    });
  }

  return nodes;
}

// ---------------------------------------------------------------------------
// Pipeline + infra components
// ---------------------------------------------------------------------------

function PipeNode({ label, active }: { label: string; active: boolean }) {
  return (
    <div
      className={`
        shrink-0 px-2.5 py-1 border transition-all duration-300
        ${active
          ? "border-[var(--color-accent)]/40 bg-[var(--color-accent-dim)]"
          : "border-[var(--color-border)] border-dashed opacity-40"
        }
      `}
    >
      <span className={`text-[10px] whitespace-nowrap ${active ? "text-[var(--color-accent)]" : "text-[var(--color-text-muted)]"}`}>
        {label}
      </span>
    </div>
  );
}

function Tag({ label, active, accent }: { label: string; active: boolean; accent?: boolean }) {
  return (
    <span
      className={`
        text-[10px] px-2 py-0.5 border transition-all duration-300
        ${active
          ? accent
            ? "border-[var(--color-accent)]/40 text-[var(--color-accent)] bg-[var(--color-accent-dim)]"
            : "border-[var(--color-border)] text-[var(--color-text-dim)]"
          : "border-[var(--color-border)] border-dashed text-[var(--color-text-muted)] opacity-40"
        }
      `}
    >
      {label}
    </span>
  );
}

function Arrow() {
  return (
    <div className="flex items-center shrink-0 px-1">
      <div className="w-3 h-px bg-[var(--color-accent)]/30" />
      <div className="w-0 h-0 border-t-[3px] border-t-transparent border-b-[3px] border-b-transparent border-l-[4px] border-l-[var(--color-accent)]/30" />
    </div>
  );
}
