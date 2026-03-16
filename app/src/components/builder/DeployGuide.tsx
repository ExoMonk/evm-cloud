import { useState } from "react";
import type { BuilderState } from "../../lib/configSchema.ts";

interface Props {
  state: BuilderState;
}

/**
 * Deployment guide — shows deployment steps, DNS, IAM, security, environments.
 * Rendered as a tab in the preview panel.
 */
export function DeployGuide({ state }: Props) {
  const isAws = state.provider === "aws";
  const isExternal = state.workloadMode === "external";

  return (
    <div className="space-y-5 overflow-auto max-h-[70vh] scrollbar-none">

      {/* Deployment steps */}
      <Section title="deploy">
        <div className="space-y-2">
          <Step n={1} done>
            <span className="text-[var(--color-text)]">configure secrets</span>
            <Code>cp secrets.auto.tfvars.example secrets.auto.tfvars</Code>
            <p className="text-[var(--color-text-muted)]">fill in real values for database credentials, SSH keys, API keys</p>
          </Step>

          <Step n={2}>
            <span className="text-[var(--color-text)]">initialize</span>
            <Code>evm-cloud init</Code>
            <p className="text-[var(--color-text-muted)]">downloads providers, configures state backend</p>
          </Step>

          <Step n={3}>
            <span className="text-[var(--color-text)]">preview changes</span>
            <Code>evm-cloud deploy --dry-run</Code>
            <p className="text-[var(--color-text-muted)]">review what will be created before spending money</p>
          </Step>

          <Step n={4}>
            <span className="text-[var(--color-text)]">deploy</span>
            <Code>evm-cloud deploy</Code>
            {isExternal && (
              <p className="text-[var(--color-text-muted)]">two-phase: terraform provisions infra, then deployer installs workloads via Helm</p>
            )}
          </Step>

          <Step n={5}>
            <span className="text-[var(--color-text)]">verify</span>
            <Code>evm-cloud status</Code>
            <p className="text-[var(--color-text-muted)]">check health of all services</p>
          </Step>
        </div>
      </Section>

      {/* DNS configuration */}
      {state.ingressMode !== "none" && (
        <Section title="dns setup">
          <DnsGuide state={state} />
        </Section>
      )}

      {/* IAM permissions */}
      {isAws && (
        <Section title="iam permissions">
          <IamGuide state={state} />
        </Section>
      )}

      {/* Security recommendations */}
      <Section title="security">
        <SecurityGuide state={state} />
      </Section>

      {/* State backend */}
      <Section title="state backend">
        <StateGuide state={state} />
      </Section>

      {/* Environments */}
      <Section title="environments">
        <EnvironmentGuide state={state} />
      </Section>

    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-sections
// ---------------------------------------------------------------------------

function DnsGuide({ state }: { state: BuilderState }) {
  const guides: Record<string, { steps: string[]; note?: string }> = {
    cloudflare: {
      steps: [
        "deploy first to get the public IP from terraform output",
        "in Cloudflare DNS, create an A record pointing your domain to the EC2/node IP",
        "set SSL/TLS mode to Full (Strict) in Cloudflare dashboard",
        "Cloudflare handles TLS termination — no certs to manage",
      ],
    },
    caddy: {
      steps: [
        "deploy first to get the public IP",
        "point your domain A record to the server IP (any DNS provider)",
        "Caddy auto-provisions Let's Encrypt certificates on first request",
        "ensure port 80 + 443 are open for ACME challenge",
      ],
      note: "use tls_staging = true first to avoid Let's Encrypt rate limits during testing",
    },
    ingress_nginx: {
      steps: [
        "deploy creates an AWS NLB/ALB automatically",
        "point your domain CNAME to the load balancer hostname",
        "cert-manager auto-provisions Let's Encrypt certificates",
        "ensure the cluster can reach Let's Encrypt ACME servers",
      ],
    },
  };

  const guide = guides[state.ingressMode];
  if (!guide) return null;

  return (
    <div className="space-y-1.5">
      {guide.steps.map((step, i) => (
        <p key={i} className="text-[11px] text-[var(--color-text-dim)]">
          <span className="text-[var(--color-accent)]">{i + 1}.</span> {step}
        </p>
      ))}
      {guide.note && (
        <p className="text-[10px] text-[var(--color-warning)] mt-2">tip: {guide.note}</p>
      )}
    </div>
  );
}

function IamGuide({ state }: { state: BuilderState }) {
  const [showPolicy, setShowPolicy] = useState(false);
  const isK8s = state.computeEngine === "k3s" || state.computeEngine === "eks";

  const services = [
    "ec2:* — instances, security groups, key pairs, EBS",
    "vpc:* — VPCs, subnets, route tables, internet gateways",
    "iam:CreateRole, iam:CreateInstanceProfile — for EC2/node roles",
    "s3:* — state backend (if configured)",
  ];
  if (state.computeEngine === "eks") services.push("eks:* — cluster, node groups, OIDC");
  if (state.databaseProfile === "managed_rds") services.push("rds:* — database instances, parameter groups");
  if (state.secretsMode === "provider") services.push("secretsmanager:* — secret creation and rotation");
  if (state.stateBackend?.backend === "s3") services.push("dynamodb:* — state locking table");
  if (isK8s) services.push("ssm:GetParameter — for k3s token exchange");

  const policy = {
    Version: "2012-10-17",
    Statement: [{
      Effect: "Allow",
      Action: [
        "ec2:*", "vpc:*", "iam:CreateRole", "iam:CreateInstanceProfile",
        "iam:PassRole", "iam:AttachRolePolicy", "iam:PutRolePolicy",
        "s3:*", "dynamodb:*",
        ...(state.computeEngine === "eks" ? ["eks:*"] : []),
        ...(state.databaseProfile === "managed_rds" ? ["rds:*"] : []),
        ...(state.secretsMode === "provider" ? ["secretsmanager:*", "kms:*"] : []),
        "ssm:GetParameter", "ssm:PutParameter",
      ],
      Resource: "*",
    }],
  };

  return (
    <div className="space-y-2">
      <p className="text-[10px] text-[var(--color-text-muted)]">
        your AWS user/role needs these permissions to run terraform apply:
      </p>
      <div className="space-y-1">
        {services.map((s, i) => (
          <p key={i} className="text-[11px] text-[var(--color-text-dim)]">
            <span className="text-[var(--color-accent)]">•</span> {s}
          </p>
        ))}
      </div>
      <button
        onClick={() => setShowPolicy(!showPolicy)}
        className="text-[10px] text-[var(--color-text-muted)] hover:text-[var(--color-accent)] transition-colors uppercase tracking-[0.1em]"
      >
        {showPolicy ? "▾ hide" : "▸ show"} IAM policy JSON
      </button>
      {showPolicy && (
        <CopyableCode content={JSON.stringify(policy, null, 2)} />
      )}
    </div>
  );
}

function SecurityGuide({ state }: { state: BuilderState }) {
  const isAws = state.provider === "aws";
  const isK8s = state.computeEngine === "k3s" || state.computeEngine === "eks";

  const items: { level: "critical" | "recommended" | "optional"; text: string }[] = [];

  // Critical
  if (!state.stateBackend) {
    items.push({ level: "critical", text: "configure remote state (S3/GCS) — local state cannot be recovered if lost" });
  }
  if (state.stateBackend?.backend === "s3" && !state.stateBackend.encrypt) {
    items.push({ level: "critical", text: "enable state encryption — state contains sensitive data" });
  }
  items.push({ level: "critical", text: "never commit secrets.auto.tfvars to git — it contains passwords and keys" });

  // Recommended
  if (isAws && state.secretsMode === "inline") {
    items.push({ level: "recommended", text: "switch secrets_mode to 'provider' (AWS Secrets Manager) for production" });
  }
  if (isK8s) {
    items.push({ level: "recommended", text: "restrict k3s_api_allowed_cidrs — don't leave 0.0.0.0/0 in production" });
  }
  items.push({ level: "recommended", text: "rotate SSH keys periodically — compromised keys mean root access" });
  if (!state.monitoring?.enabled && isK8s) {
    items.push({ level: "recommended", text: "enable monitoring for production — you need visibility into indexer lag and errors" });
  }

  // Optional
  if (isAws) {
    items.push({ level: "optional", text: "enable VPC endpoints to keep traffic off the public internet" });
  }
  items.push({ level: "optional", text: "pin container images to specific tags (not :latest) for reproducibility" });

  const levelColor = { critical: "text-[var(--color-error)]", recommended: "text-[var(--color-warning)]", optional: "text-[var(--color-text-muted)]" };
  const levelIcon = { critical: "✕", recommended: "⚠", optional: "ℹ" };

  return (
    <div className="space-y-1.5">
      {items.map((item, i) => (
        <p key={i} className={`text-[11px] ${levelColor[item.level]}`}>
          {levelIcon[item.level]} <span className="text-[var(--color-text-dim)]">{item.text}</span>
        </p>
      ))}
    </div>
  );
}

function StateGuide({ state }: { state: BuilderState }) {
  if (!state.stateBackend) {
    return (
      <div className="space-y-2">
        <p className="text-[11px] text-[var(--color-warning)]">
          ⚠ using local state — fine for testing, not for production.
        </p>
        <p className="text-[10px] text-[var(--color-text-muted)]">
          configure S3 or GCS in the advanced section (step 6) to enable team collaboration, state locking, and recovery.
        </p>
        <Code>evm-cloud bootstrap</Code>
        <p className="text-[10px] text-[var(--color-text-muted)]">
          creates the S3 bucket + DynamoDB lock table automatically.
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-1.5">
      <p className="text-[11px] text-[var(--color-accent)]">
        ● {state.stateBackend.backend.toUpperCase()} remote state configured
      </p>
      <p className="text-[10px] text-[var(--color-text-dim)]">
        bucket: {state.stateBackend.bucket}
      </p>
      {state.stateBackend.backend === "s3" && (
        <p className="text-[10px] text-[var(--color-text-dim)]">
          lock table: {state.stateBackend.dynamodbTable} · encrypt: {state.stateBackend.encrypt ? "yes" : "no"}
        </p>
      )}
      <p className="text-[10px] text-[var(--color-text-muted)] mt-1">
        state is versioned in S3 — recover previous versions from the AWS console if needed.
      </p>
    </div>
  );
}

function EnvironmentGuide({ state: _state }: { state: BuilderState }) {
  return (
    <div className="space-y-2">
      <p className="text-[10px] text-[var(--color-text-muted)]">
        evm-cloud supports multi-environment deployment with isolated state per env.
      </p>
      <div className="space-y-1">
        <Code>evm-cloud env add staging</Code>
        <Code>evm-cloud env add production</Code>
        <Code>evm-cloud env list</Code>
      </div>
      <p className="text-[10px] text-[var(--color-text-muted)]">
        each environment gets its own terraform state, backend config, and deploy lock.
        use <span className="text-[var(--color-text-dim)]">--env staging</span> on any command to target a specific environment.
      </p>
      <p className="text-[10px] text-[var(--color-text-muted)] mt-1">
        recommended pattern: start with a single env (default), then add staging → production when ready.
      </p>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Shared components
// ---------------------------------------------------------------------------

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-3">
        // {title}
      </p>
      {children}
    </div>
  );
}

function Step({ n, done, children }: { n: number; done?: boolean; children: React.ReactNode }) {
  return (
    <div className="flex gap-3 text-[11px]">
      <span className={`w-4 shrink-0 text-center ${done ? "text-[var(--color-accent)]" : "text-[var(--color-text-muted)]"}`}>
        {n}
      </span>
      <div className="space-y-0.5">{children}</div>
    </div>
  );
}

function Code({ children }: { children: string }) {
  return (
    <code className="block text-[11px] text-[var(--color-accent)] bg-[rgba(0,0,0,0.3)] px-2 py-1 mt-0.5">
      $ {children}
    </code>
  );
}

function CopyableCode({ content }: { content: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="relative">
      <pre className="text-[10px] text-[var(--color-text-dim)] bg-[rgba(0,0,0,0.3)] p-3 overflow-x-auto scrollbar-none max-h-48">
        {content}
      </pre>
      <button
        onClick={() => { navigator.clipboard.writeText(content); setCopied(true); setTimeout(() => setCopied(false), 1500); }}
        className="absolute top-2 right-2 text-[9px] text-[var(--color-text-muted)] hover:text-[var(--color-accent)] transition-colors uppercase tracking-[0.1em]"
      >
        {copied ? "copied" : "copy"}
      </button>
    </div>
  );
}
