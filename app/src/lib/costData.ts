/**
 * Cost estimation data from documentation/docs/pages/docs/cost-estimates.mdx
 *
 * Shows estimated monthly AWS costs based on infrastructure choices.
 */

import type { BuilderState, InfraProfile } from "./configSchema.ts";

export interface CostLine {
  component: string;
  cost: string;
  note?: string;
}

export interface CostWarning {
  severity: "warning" | "info";
  message: string;
}

export interface CostEstimate {
  monthlyMin: number;
  monthlyMax: number;
  breakdown: CostLine[];
  warnings: CostWarning[];
}

const BASE_COSTS: Record<InfraProfile, { min: number; max: number }> = {
  aws_simple: { min: 15, max: 35 },
  aws_budget_k8s: { min: 35, max: 50 },
  aws_managed_k8s: { min: 110, max: 140 },
  vps_docker: { min: 5, max: 20 },
  vps_k8s: { min: 0, max: 0 },
};

export function estimateCost(state: BuilderState): CostEstimate {
  const breakdown: CostLine[] = [];
  const warnings: CostWarning[] = [];

  if (!state.infraProfile) {
    return { monthlyMin: 0, monthlyMax: 0, breakdown: [], warnings: [] };
  }

  const base = BASE_COSTS[state.infraProfile];
  let min = base.min;
  let max = base.max;

  // Compute
  if (state.provider === "aws" && state.instanceType) {
    breakdown.push({
      component: `EC2 (${state.instanceType})`,
      cost: `$${min}-${max}/mo`,
    });
  } else if (state.provider === "bare_metal") {
    breakdown.push({
      component: "VPS / Server",
      cost: "Your cost",
      note: "Not included in estimate",
    });
  }

  // EKS control plane
  if (state.computeEngine === "eks") {
    breakdown.push({ component: "EKS control plane", cost: "$73/mo" });
    warnings.push({
      severity: "info",
      message: "EKS control plane costs $73/mo flat, before any worker nodes.",
    });
  }

  // Database
  if (state.databaseProfile === "managed_rds") {
    breakdown.push({ component: "RDS PostgreSQL", cost: "$13-45/mo" });
    min += 13;
    max += 45;
  } else if (state.databaseProfile.startsWith("byodb")) {
    breakdown.push({
      component: `BYO ${state.databaseProfile.includes("clickhouse") ? "ClickHouse" : "PostgreSQL"}`,
      cost: "$0",
      note: "Your infrastructure",
    });
  }

  // Monitoring
  if (state.monitoring?.enabled) {
    breakdown.push({ component: "Monitoring (Grafana + Prometheus)", cost: "~$5-15/mo" });
    min += 5;
    max += 15;
  }

  // Cost warnings
  if (state.provider === "aws" && state.networking?.enableVpcEndpoints) {
    warnings.push({
      severity: "warning",
      message: "Each VPC endpoint costs ~$7.20/mo + data processing fees.",
    });
  }

  warnings.push({
    severity: "info",
    message: "Data transfer out of AWS is $0.09/GB after the first 100GB/mo. Not included in estimate.",
  });

  if (state.provider === "aws") {
    warnings.push({
      severity: "warning",
      message: "NAT Gateway (if enabled) adds ~$32/mo + $0.045/GB processed.",
    });
  }

  return { monthlyMin: min, monthlyMax: max, breakdown, warnings };
}
