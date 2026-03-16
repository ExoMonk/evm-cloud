/**
 * TypeScript port of cli/src/config/validation.rs
 *
 * IMPORTANT: When validation rules change in the Rust CLI,
 * this file must be updated. Schema contract test (M1) catches drift.
 */

import {
  type BuilderState,
  COMPUTE_ENGINES_BY_PROVIDER,
  INGRESS_MODES_BY_ENGINE,
} from "./configSchema.ts";

export interface ValidationIssue {
  field: string;
  severity: "error" | "warning" | "info";
  message: string;
}

export function validate(state: BuilderState): ValidationIssue[] {
  const issues: ValidationIssue[] = [];

  // --- Errors (block export) ---

  // 1. Project name non-empty
  if (!state.projectName.trim()) {
    issues.push({
      field: "project.name",
      severity: "error",
      message: "Project name is required.",
    });
  }

  // 2. AWS provider requires region
  if (state.provider === "aws" && !state.region) {
    issues.push({
      field: "project.region",
      severity: "error",
      message: "AWS region is required when using AWS provider.",
    });
  }

  // 3. Compute engine valid for provider
  const validEngines = COMPUTE_ENGINES_BY_PROVIDER[state.provider];
  if (!validEngines.includes(state.computeEngine)) {
    issues.push({
      field: "compute.engine",
      severity: "error",
      message: `${state.computeEngine} is not valid for ${state.provider} provider. Valid: ${validEngines.join(", ")}.`,
    });
  }

  // 4. Ingress mode valid for engine
  const validIngress = INGRESS_MODES_BY_ENGINE[state.computeEngine];
  if (!validIngress.includes(state.ingressMode)) {
    issues.push({
      field: "ingress.mode",
      severity: "error",
      message: `${state.ingressMode} is not valid for ${state.computeEngine}. Valid: ${validIngress.join(", ")}.`,
    });
  }

  // 5. At least one chain
  if (state.chains.length === 0) {
    issues.push({
      field: "indexer.chains",
      severity: "error",
      message: "Select at least one chain.",
    });
  }

  // 6. RPC endpoint per chain
  for (const chain of state.chains) {
    if (!state.rpcEndpoints[chain]?.trim()) {
      issues.push({
        field: `rpc.endpoints.${chain}`,
        severity: "error",
        message: `RPC endpoint required for ${chain}.`,
      });
    }
  }

  // 7. eRPC hostname is optional — no error if empty

  // 8. Caddy/ingress_nginx requires TLS email
  if (
    (state.ingressMode === "caddy" || state.ingressMode === "ingress_nginx") &&
    !state.tlsEmail.trim()
  ) {
    issues.push({
      field: "ingress.tls_email",
      severity: "error",
      message: "TLS email is required for Let's Encrypt certificate provisioning.",
    });
  }

  // 9. Custom indexer requires image
  if (state.indexerType === "custom" && !state.customIndexerImage.trim()) {
    issues.push({
      field: "containers.indexer_image",
      severity: "error",
      message: "Custom indexer image is required when indexer type is 'custom'.",
    });
  }

  // 10. State backend fields non-empty when configured
  if (state.stateBackend) {
    if (!state.stateBackend.bucket.trim()) {
      issues.push({
        field: "state.bucket",
        severity: "error",
        message: "State backend bucket name is required.",
      });
    }
    if (state.stateBackend.backend === "s3" && !state.stateBackend.dynamodbTable.trim()) {
      issues.push({
        field: "state.dynamodb_table",
        severity: "error",
        message: "DynamoDB lock table name is required for S3 backend.",
      });
    }
    if (!state.stateBackend.region.trim()) {
      issues.push({
        field: "state.region",
        severity: "error",
        message: "State backend region is required.",
      });
    }
  }

  // --- Warnings (non-blocking) ---

  // No state backend on AWS
  if (state.provider === "aws" && !state.stateBackend) {
    issues.push({
      field: "state",
      severity: "warning",
      message: "No remote state configured. Without it, you cannot collaborate or recover from disk loss. Recommended for production.",
    });
  }

  // EKS cost warning
  if (state.computeEngine === "eks") {
    issues.push({
      field: "compute.engine",
      severity: "info",
      message: "EKS control plane costs $73/mo flat, before any worker nodes.",
    });
  }

  // Small instance for multi-chain
  if (
    state.chains.length > 2 &&
    state.instanceType &&
    ["t3.nano", "t3.micro"].includes(state.instanceType)
  ) {
    issues.push({
      field: "compute.instance_type",
      severity: "warning",
      message: `${state.instanceType} may be undersized for ${state.chains.length} chains. Consider t3.medium or larger.`,
    });
  }

  // No monitoring in production-looking setup
  if (
    state.domain &&
    state.chains.length > 1 &&
    (!state.monitoring || !state.monitoring.enabled)
  ) {
    issues.push({
      field: "monitoring",
      severity: "warning",
      message: "Monitoring is disabled. Recommended for production deployments with multiple chains.",
    });
  }

  return issues;
}

/** Returns only errors (severity === "error") */
export function hasErrors(state: BuilderState): boolean {
  return validate(state).some((i) => i.severity === "error");
}
