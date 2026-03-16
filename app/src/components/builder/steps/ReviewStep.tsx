import { useState, type Dispatch } from "react";
import type {
  BuilderState,
  BuilderAction,
  SecretsMode,
  CustomService,
  StreamingMode,
  S3State,
  GcsState,
} from "../../../lib/configSchema.ts";
import { AWS_REGIONS } from "../../../lib/configSchema.ts";
import { CornerCard } from "../../ui/CornerCard.tsx";

interface Props {
  state: BuilderState;
  dispatch: Dispatch<BuilderAction>;
}

export function ReviewStep({ state, dispatch }: Props) {
  const [openSection, setOpenSection] = useState<string | null>(null);

  const toggle = (section: string) => {
    setOpenSection(openSection === section ? null : section);
  };

  return (
    <div className="space-y-5">
      {/* Summary table */}
      <CornerCard className="p-5">
        <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-3">
          // summary
        </p>
        <div className="space-y-2 text-[12px]">
          <Row label="project" value={state.projectName} />
          <Row label="infra" value={state.infraProfile ?? "—"} />
          <Row label="provider" value={`${state.provider} / ${state.computeEngine}`} />
          {state.provider === "aws" && (
            <>
              <Row label="region" value={state.region} />
              <Row label="instance" value={state.instanceType} />
            </>
          )}
          <Row label="database" value={state.databaseProfile} />
          <Row label="chains" value={state.chains.join(", ") || "—"} accent={state.chains.length > 0} />
          <Row label="ingress" value={state.ingressMode} />
          {state.domain && <Row label="domain" value={state.domain} />}
          <Row label="secrets" value={state.secretsMode} />
          <Row label="workload" value={state.workloadMode} />
        </div>
      </CornerCard>

      {/* Advanced sections — real accordions */}
      <div>
        <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-3">
          // advanced
        </p>
        <div className="space-y-1.5">

          {/* Secrets */}
          <Accordion label="secrets" open={openSection === "secrets"} onToggle={() => toggle("secrets")}>
            <SecretsSection state={state} dispatch={dispatch} />
          </Accordion>

          {/* Monitoring (k8s only) */}
          {(state.computeEngine === "k3s" || state.computeEngine === "eks") && (
            <Accordion label="monitoring" open={openSection === "monitoring"} onToggle={() => toggle("monitoring")}>
              <MonitoringSection state={state} dispatch={dispatch} />
            </Accordion>
          )}

          {/* Networking (AWS only) */}
          {state.provider === "aws" && (
            <Accordion label="networking" open={openSection === "networking"} onToggle={() => toggle("networking")}>
              <NetworkingSection state={state} dispatch={dispatch} />
            </Accordion>
          )}

          {/* State Backend */}
          <Accordion label="state backend" open={openSection === "state"} onToggle={() => toggle("state")}>
            <StateBackendSection state={state} dispatch={dispatch} />
          </Accordion>

          {/* Streaming */}
          <Accordion label="streaming" open={openSection === "streaming"} onToggle={() => toggle("streaming")}>
            <StreamingSection state={state} dispatch={dispatch} />
          </Accordion>

          {/* Containers */}
          <Accordion label="containers" open={openSection === "containers"} onToggle={() => toggle("containers")}>
            <ContainersSection state={state} dispatch={dispatch} />
          </Accordion>

          {/* Extra Env */}
          <Accordion label="extra env" open={openSection === "env"} onToggle={() => toggle("env")}>
            <ExtraEnvSection state={state} dispatch={dispatch} />
          </Accordion>

          {/* Custom Services */}
          <Accordion label={`custom services${state.customServices.length > 0 ? ` (${state.customServices.length})` : ""}`} open={openSection === "services"} onToggle={() => toggle("services")}>
            <CustomServicesSection state={state} dispatch={dispatch} />
          </Accordion>

        </div>
      </div>

      {/* Deploy workflow */}
      <CornerCard accent className="p-5">
        <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-3">
          // after download
        </p>
        <div className="space-y-1.5 text-[12px] text-[var(--color-text-dim)]">
          <p><span className="text-[var(--color-accent)]">$</span> cd {state.projectName}</p>
          <p><span className="text-[var(--color-accent)]">$</span> cp secrets.auto.tfvars.example secrets.auto.tfvars</p>
          <p className="text-[var(--color-text-muted)] pl-3">edit secrets.auto.tfvars with real values</p>
          <p><span className="text-[var(--color-accent)]">$</span> evm-cloud init</p>
          <p><span className="text-[var(--color-accent)]">$</span> evm-cloud deploy --dry-run</p>
          <p><span className="text-[var(--color-accent)]">$</span> evm-cloud deploy</p>
        </div>
      </CornerCard>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Shared components
// ---------------------------------------------------------------------------

function Row({ label, value, accent = false }: { label: string; value: string; accent?: boolean }) {
  return (
    <div className="flex justify-between items-baseline">
      <span className="text-[var(--color-text-muted)]">{label}</span>
      <span className={accent ? "text-[var(--color-accent)]" : "text-[var(--color-text-dim)]"}>
        {value}
      </span>
    </div>
  );
}

function Accordion({ label, open, onToggle, children }: { label: string; open: boolean; onToggle: () => void; children: React.ReactNode }) {
  return (
    <div className={`border transition-colors ${open ? "border-[var(--color-accent)]/30" : "border-[var(--color-border)] hover:border-[var(--color-border-hover)]"}`}>
      <button
        onClick={onToggle}
        className="w-full px-4 py-2.5 flex items-center justify-between text-left"
      >
        <span className={`text-[11px] uppercase tracking-[0.15em] ${open ? "text-[var(--color-accent)]" : "text-[var(--color-text-muted)]"}`}>
          {open ? "▾" : "▸"} {label}
        </span>
      </button>
      {open && (
        <div className="px-4 pb-4 border-t border-[var(--color-border)]">
          <div className="pt-3 space-y-3">
            {children}
          </div>
        </div>
      )}
    </div>
  );
}

function FieldInput({ label, value, onChange, placeholder, type = "text" }: {
  label: string; value: string; onChange: (v: string) => void; placeholder?: string; type?: string;
}) {
  return (
    <div>
      <label className="block text-[10px] text-[var(--color-text-muted)] mb-1">{label}</label>
      <input
        type={type}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="w-full px-3 py-2 bg-transparent border border-[var(--color-border)] text-[12px] text-[var(--color-text)] placeholder-[var(--color-text-muted)]/50 focus:outline-none focus:border-[var(--color-accent)]/50 transition-colors"
      />
    </div>
  );
}

function FieldToggle({ label, value, onChange, description }: {
  label: string; value: boolean; onChange: (v: boolean) => void; description?: string;
}) {
  return (
    <button
      onClick={() => onChange(!value)}
      className="w-full flex items-center justify-between px-3 py-2 border border-[var(--color-border)] hover:border-[var(--color-border-hover)] transition-colors text-left"
    >
      <div>
        <span className="text-[11px] text-[var(--color-text-dim)]">{label}</span>
        {description && <p className="text-[10px] text-[var(--color-text-muted)]">{description}</p>}
      </div>
      <span className={`text-[11px] ${value ? "text-[var(--color-accent)]" : "text-[var(--color-text-muted)]"}`}>
        {value ? "● on" : "○ off"}
      </span>
    </button>
  );
}

function FieldSelect({ label, value, onChange, options }: {
  label: string; value: string; onChange: (v: string) => void; options: { value: string; label: string }[];
}) {
  return (
    <div>
      <label className="block text-[10px] text-[var(--color-text-muted)] mb-1">{label}</label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full px-3 py-2 bg-transparent border border-[var(--color-border)] text-[12px] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-accent)]/50 transition-colors"
      >
        {options.map((o) => (
          <option key={o.value} value={o.value} className="bg-[var(--color-bg)]">{o.label}</option>
        ))}
      </select>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Section content
// ---------------------------------------------------------------------------

function SecretsSection({ state, dispatch }: { state: BuilderState; dispatch: Dispatch<BuilderAction> }) {
  return (
    <>
      <FieldSelect
        label="secrets mode"
        value={state.secretsMode}
        onChange={(v) => dispatch({ type: "SET_SECRETS_MODE", mode: v as SecretsMode })}
        options={[
          { value: "inline", label: "inline — secrets in tfvars (simplest)" },
          { value: "provider", label: "provider — AWS Secrets Manager" },
          { value: "external", label: "external — External Secrets Operator" },
        ]}
      />
      <p className="text-[10px] text-[var(--color-text-muted)]">
        {state.secretsMode === "inline" && "Secrets stored in secrets.auto.tfvars (gitignored). Simple but requires manual management."}
        {state.secretsMode === "provider" && "Secrets stored in AWS Secrets Manager. Synced to K8s via ESO. Recommended for production."}
        {state.secretsMode === "external" && "Bring your own external secret store (e.g., HashiCorp Vault)."}
      </p>
    </>
  );
}

function MonitoringSection({ state, dispatch }: { state: BuilderState; dispatch: Dispatch<BuilderAction> }) {
  const monitoring = state.monitoring ?? { enabled: false };

  const update = (patch: Partial<typeof monitoring>) => {
    dispatch({ type: "SET_MONITORING", config: { ...monitoring, ...patch } });
  };

  return (
    <>
      <FieldToggle
        label="monitoring stack"
        value={monitoring.enabled}
        onChange={(v) => update({ enabled: v })}
        description="Grafana + Prometheus + Alertmanager"
      />
      {monitoring.enabled && (
        <>
          <FieldInput
            label="grafana hostname"
            value={monitoring.grafanaHostname ?? ""}
            onChange={(v) => update({ grafanaHostname: v })}
            placeholder="grafana.example.com"
          />
          <FieldInput
            label="alertmanager slack channel"
            value={monitoring.alertmanagerSlackChannel ?? ""}
            onChange={(v) => update({ alertmanagerSlackChannel: v })}
            placeholder="#alerts"
          />
          <FieldToggle
            label="loki log aggregation"
            value={monitoring.lokiEnabled ?? false}
            onChange={(v) => update({ lokiEnabled: v })}
            description="Centralized log collection via Loki + Promtail"
          />
        </>
      )}
    </>
  );
}

function NetworkingSection({ state, dispatch }: { state: BuilderState; dispatch: Dispatch<BuilderAction> }) {
  const networking = state.networking ?? {};

  const update = (patch: Partial<typeof networking>) => {
    dispatch({ type: "SET_NETWORKING", config: { ...networking, ...patch } });
  };

  return (
    <>
      <FieldInput
        label="VPC CIDR"
        value={networking.vpcCidr ?? "10.42.0.0/16"}
        onChange={(v) => update({ vpcCidr: v })}
        placeholder="10.42.0.0/16"
      />
      <FieldToggle
        label="VPC endpoints"
        value={networking.enableVpcEndpoints ?? false}
        onChange={(v) => update({ enableVpcEndpoints: v })}
        description="Enable VPC endpoints for AWS services (~$7.20/mo each)"
      />
      <FieldSelect
        label="environment"
        value={networking.environment ?? "dev"}
        onChange={(v) => update({ environment: v })}
        options={[
          { value: "dev", label: "dev" },
          { value: "staging", label: "staging" },
          { value: "production", label: "production" },
        ]}
      />
    </>
  );
}

function StateBackendSection({ state, dispatch }: { state: BuilderState; dispatch: Dispatch<BuilderAction> }) {
  const backend = state.stateBackend;
  const hasBackend = backend !== null;

  const setS3 = () => {
    dispatch({
      type: "SET_STATE_BACKEND",
      backend: {
        backend: "s3",
        bucket: `${state.projectName}-tfstate`,
        dynamodbTable: `${state.projectName}-tflock`,
        region: state.region || "us-east-1",
        encrypt: true,
      },
    });
  };

  const setGcs = () => {
    dispatch({
      type: "SET_STATE_BACKEND",
      backend: { backend: "gcs", bucket: `${state.projectName}-tfstate`, region: "US" },
    });
  };

  const clear = () => {
    dispatch({ type: "SET_STATE_BACKEND", backend: null });
  };

  if (!hasBackend) {
    return (
      <>
        <p className="text-[10px] text-[var(--color-warning)]">
          ⚠ No remote state. For production, configure S3 or GCS to enable collaboration and recovery.
        </p>
        <div className="flex gap-2">
          <button onClick={setS3} className="flex-1 px-3 py-2 border border-[var(--color-border)] text-[11px] text-[var(--color-text-dim)] hover:border-[var(--color-accent)]/40 hover:text-[var(--color-accent)] transition-colors">
            + S3 backend
          </button>
          <button onClick={setGcs} className="flex-1 px-3 py-2 border border-[var(--color-border)] text-[11px] text-[var(--color-text-dim)] hover:border-[var(--color-accent)]/40 hover:text-[var(--color-accent)] transition-colors">
            + GCS backend
          </button>
        </div>
      </>
    );
  }

  if (backend.backend === "s3") {
    const s3 = backend as S3State;
    const updateS3 = (patch: Partial<S3State>) => {
      dispatch({ type: "SET_STATE_BACKEND", backend: { ...s3, ...patch } });
    };
    return (
      <>
        <div className="flex items-center justify-between">
          <span className="text-[11px] text-[var(--color-accent)]">S3 backend</span>
          <button onClick={clear} className="text-[10px] text-[var(--color-text-muted)] hover:text-[var(--color-error)] transition-colors">remove</button>
        </div>
        <FieldInput label="bucket" value={s3.bucket} onChange={(v) => updateS3({ bucket: v })} placeholder="my-project-tfstate" />
        <FieldInput label="DynamoDB lock table" value={s3.dynamodbTable} onChange={(v) => updateS3({ dynamodbTable: v })} placeholder="my-project-tflock" />
        <FieldSelect
          label="region"
          value={s3.region}
          onChange={(v) => updateS3({ region: v })}
          options={AWS_REGIONS.map((r) => ({ value: r, label: r }))}
        />
        <FieldToggle label="encrypt" value={s3.encrypt} onChange={(v) => updateS3({ encrypt: v })} description="Encrypt state at rest in S3" />
      </>
    );
  }

  // GCS
  const gcs = backend as GcsState;
  const updateGcs = (patch: Partial<GcsState>) => {
    dispatch({ type: "SET_STATE_BACKEND", backend: { ...gcs, ...patch } });
  };
  return (
    <>
      <div className="flex items-center justify-between">
        <span className="text-[11px] text-[var(--color-accent)]">GCS backend</span>
        <button onClick={clear} className="text-[10px] text-[var(--color-text-muted)] hover:text-[var(--color-error)] transition-colors">remove</button>
      </div>
      <FieldInput label="bucket" value={gcs.bucket} onChange={(v) => updateGcs({ bucket: v })} placeholder="my-project-tfstate" />
      <FieldSelect
        label="region"
        value={gcs.region}
        onChange={(v) => updateGcs({ region: v })}
        options={[
          { value: "US", label: "US (multi-region)" },
          { value: "EU", label: "EU (multi-region)" },
          { value: "us-central1", label: "us-central1" },
          { value: "europe-west1", label: "europe-west1" },
          { value: "asia-east1", label: "asia-east1" },
        ]}
      />
    </>
  );
}

function StreamingSection({ state, dispatch }: { state: BuilderState; dispatch: Dispatch<BuilderAction> }) {
  const mode = state.streaming?.mode ?? "disabled";

  return (
    <>
      <FieldSelect
        label="streaming mode"
        value={mode}
        onChange={(v) => dispatch({ type: "SET_STREAMING", config: { mode: v as StreamingMode } })}
        options={[
          { value: "disabled", label: "disabled" },
          { value: "kafka", label: "kafka (MSK) — ordered event streaming" },
          { value: "sns-sqs", label: "SNS-SQS — AWS-native fanout" },
          { value: "cdc", label: "CDC — change data capture from DB" },
        ]}
      />
      {mode === "kafka" && (
        <p className="text-[10px] text-[var(--color-warning)]">
          ⚠ MSK minimum cost ~$150/mo per broker. Evaluate if you need ordered streaming before enabling.
        </p>
      )}
    </>
  );
}

function ContainersSection({ state, dispatch }: { state: BuilderState; dispatch: Dispatch<BuilderAction> }) {
  const containers = state.containers ?? {};

  const update = (patch: Partial<typeof containers>) => {
    dispatch({ type: "SET_CONTAINERS", config: { ...containers, ...patch } });
  };

  return (
    <>
      <FieldInput
        label="eRPC image"
        value={containers.rpcProxyImage ?? ""}
        onChange={(v) => update({ rpcProxyImage: v })}
        placeholder="ghcr.io/erpc/erpc:latest"
      />
      <FieldInput
        label="indexer image"
        value={containers.indexerImage ?? ""}
        onChange={(v) => update({ indexerImage: v })}
        placeholder="ghcr.io/joshstevens19/rindexer:latest"
      />
      <p className="text-[10px] text-[var(--color-text-muted)]">
        Leave blank to use defaults. Override for custom builds or pinned versions.
      </p>
    </>
  );
}

function ExtraEnvSection({ state, dispatch }: { state: BuilderState; dispatch: Dispatch<BuilderAction> }) {
  const entries = Object.entries(state.extraEnv);

  const addEntry = () => {
    const key = `ENV_VAR_${entries.length + 1}`;
    dispatch({ type: "SET_EXTRA_ENV", env: { ...state.extraEnv, [key]: "" } });
  };

  const updateKey = (oldKey: string, newKey: string) => {
    const env = { ...state.extraEnv };
    const val = env[oldKey] ?? "";
    delete env[oldKey];
    env[newKey] = val;
    dispatch({ type: "SET_EXTRA_ENV", env });
  };

  const updateValue = (key: string, value: string) => {
    dispatch({ type: "SET_EXTRA_ENV", env: { ...state.extraEnv, [key]: value } });
  };

  const removeEntry = (key: string) => {
    const env = { ...state.extraEnv };
    delete env[key];
    dispatch({ type: "SET_EXTRA_ENV", env });
  };

  return (
    <>
      {entries.length === 0 && (
        <p className="text-[10px] text-[var(--color-text-muted)]">
          No extra environment variables. Add key-value pairs for the indexer container.
        </p>
      )}
      {entries.map(([key, value]) => (
        <div key={key} className="flex gap-2 items-start">
          <input
            type="text"
            value={key}
            onChange={(e) => updateKey(key, e.target.value)}
            placeholder="KEY"
            className="w-1/3 px-2 py-1.5 bg-transparent border border-[var(--color-border)] text-[11px] text-[var(--color-text)] placeholder-[var(--color-text-muted)]/50 focus:outline-none focus:border-[var(--color-accent)]/50"
          />
          <input
            type="text"
            value={value}
            onChange={(e) => updateValue(key, e.target.value)}
            placeholder="value"
            className="flex-1 px-2 py-1.5 bg-transparent border border-[var(--color-border)] text-[11px] text-[var(--color-text)] placeholder-[var(--color-text-muted)]/50 focus:outline-none focus:border-[var(--color-accent)]/50"
          />
          <button
            onClick={() => removeEntry(key)}
            className="px-2 py-1.5 text-[10px] text-[var(--color-text-muted)] hover:text-[var(--color-error)] transition-colors"
          >
            ✕
          </button>
        </div>
      ))}
      <button
        onClick={addEntry}
        className="w-full px-3 py-2 border border-dashed border-[var(--color-border)] text-[11px] text-[var(--color-text-muted)] hover:border-[var(--color-accent)]/40 hover:text-[var(--color-accent)] transition-colors"
      >
        + add variable
      </button>
    </>
  );
}

function CustomServicesSection({ state, dispatch }: { state: BuilderState; dispatch: Dispatch<BuilderAction> }) {
  const services = state.customServices;

  const newService = (): CustomService => ({
    name: `service-${services.length + 1}`,
    image: "",
    port: 3000,
    healthPath: "/health",
    replicas: 1,
    cpuRequest: "250m",
    cpuLimit: "500m",
    memoryRequest: "256Mi",
    memoryLimit: "512Mi",
    env: {},
    ingressHostname: "",
    nodeRole: "",
  });

  const add = () => dispatch({ type: "SET_CUSTOM_SERVICES", services: [...services, newService()] });

  const update = (idx: number, patch: Partial<CustomService>) => {
    const updated = services.map((s, i) => i === idx ? { ...s, ...patch } : s);
    dispatch({ type: "SET_CUSTOM_SERVICES", services: updated });
  };

  const remove = (idx: number) => {
    dispatch({ type: "SET_CUSTOM_SERVICES", services: services.filter((_, i) => i !== idx) });
  };

  const [expanded, setExpanded] = useState<number | null>(null);

  return (
    <>
      {services.length === 0 && (
        <p className="text-[10px] text-[var(--color-text-muted)]">
          no custom services. add a containerized service to deploy alongside the indexer stack.
        </p>
      )}

      {services.map((svc, idx) => (
        <div key={idx} className="border border-[var(--color-border)] transition-colors">
          <button
            onClick={() => setExpanded(expanded === idx ? null : idx)}
            className="w-full px-3 py-2 flex items-center justify-between text-left"
          >
            <span className="text-[11px] text-[var(--color-text)]">
              {svc.name || `service-${idx + 1}`}
              {svc.image && <span className="text-[var(--color-text-muted)] ml-2">({svc.image.split("/").pop()})</span>}
            </span>
            <div className="flex items-center gap-2">
              <span className="text-[10px] text-[var(--color-text-muted)]">:{svc.port}</span>
              <button
                onClick={(e) => { e.stopPropagation(); remove(idx); }}
                className="text-[10px] text-[var(--color-text-muted)] hover:text-[var(--color-error)] transition-colors"
              >
                ✕
              </button>
            </div>
          </button>

          {expanded === idx && (
            <div className="px-3 pb-3 border-t border-[var(--color-border)] space-y-2 pt-2">
              <div className="grid grid-cols-2 gap-2">
                <FieldInput label="name" value={svc.name} onChange={(v) => update(idx, { name: v })} placeholder="my-api" />
                <FieldInput label="port" value={String(svc.port)} onChange={(v) => update(idx, { port: parseInt(v) || 3000 })} placeholder="3000" type="number" />
              </div>
              <FieldInput label="image" value={svc.image} onChange={(v) => update(idx, { image: v })} placeholder="ghcr.io/myorg/my-api:latest" />
              <FieldInput label="health path" value={svc.healthPath} onChange={(v) => update(idx, { healthPath: v })} placeholder="/health" />

              <div className="grid grid-cols-2 gap-2">
                <FieldInput label="cpu request" value={svc.cpuRequest} onChange={(v) => update(idx, { cpuRequest: v })} placeholder="250m" />
                <FieldInput label="cpu limit" value={svc.cpuLimit} onChange={(v) => update(idx, { cpuLimit: v })} placeholder="500m" />
              </div>
              <div className="grid grid-cols-2 gap-2">
                <FieldInput label="memory request" value={svc.memoryRequest} onChange={(v) => update(idx, { memoryRequest: v })} placeholder="256Mi" />
                <FieldInput label="memory limit" value={svc.memoryLimit} onChange={(v) => update(idx, { memoryLimit: v })} placeholder="512Mi" />
              </div>
              <div className="grid grid-cols-2 gap-2">
                <FieldInput label="replicas" value={String(svc.replicas)} onChange={(v) => update(idx, { replicas: parseInt(v) || 1 })} placeholder="1" type="number" />
                <FieldInput label="node role" value={svc.nodeRole} onChange={(v) => update(idx, { nodeRole: v })} placeholder="optional" />
              </div>
              {state.ingressMode !== "none" && (
                <FieldInput label="ingress hostname" value={svc.ingressHostname} onChange={(v) => update(idx, { ingressHostname: v })} placeholder="api.example.com" />
              )}
            </div>
          )}
        </div>
      ))}

      <button
        onClick={add}
        className="w-full px-3 py-2 border border-dashed border-[var(--color-border)] text-[11px] text-[var(--color-text-muted)] hover:border-[var(--color-accent)]/40 hover:text-[var(--color-accent)] transition-colors"
      >
        + add service
      </button>
    </>
  );
}
