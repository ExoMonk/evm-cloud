import React from "react";

// ─── Data ────────────────────────────────────────────────────────
// Visualizes what Terraform created after `terraform apply`.

type Service = {
  label: string;
  sub: string;
  color: string;
};

type ExternalLink = {
  label: string;
  direction: "in" | "out";
};

type InfraBox = {
  title: string;
  subtitle: string;
  color: string;
  services: Service[];
  external: ExternalLink[];
};

const infra: InfraBox = {
  title: "AWS VPC",
  subtitle: "EC2 Instance (t3.micro — free tier)",
  color: "#0ea5e9",
  services: [
    {
      label: "eRPC Proxy",
      sub: "Aggregates upstream RPCs with failover + caching",
      color: "#0ea5e9",
    },
    {
      label: "rindexer",
      sub: "Reads blocks via eRPC, decodes events, writes to DB",
      color: "#10b981",
    },
    {
      label: "Secrets Manager",
      sub: "DB credentials injected at runtime via .env",
      color: "#8b5cf6",
    },
  ],
  external: [
    { label: "Upstream RPCs (Infura, Alchemy, public)", direction: "in" },
    { label: "ClickHouse Cloud (your database)", direction: "out" },
  ],
};

// ─── Styles ──────────────────────────────────────────────────────

const s = {
  container: {
    fontFamily: "var(--vocs-fontFamily_default, system-ui, sans-serif)",
    fontSize: "13px",
    lineHeight: 1.4,
    display: "flex",
    flexDirection: "column" as const,
    gap: "6px",
  },
  // External connections
  external: (direction: "in" | "out") => ({
    display: "flex",
    alignItems: "center",
    gap: "8px",
    padding: "6px 12px",
    borderRadius: "6px",
    backgroundColor: "var(--vocs-color_background3, #1a191b)",
    border: "1px solid var(--vocs-color_border, #444)",
    fontSize: "12px",
    color: "var(--vocs-color_text, #ccc)",
  }),
  externalArrow: (direction: "in" | "out") => ({
    color: direction === "in" ? "#0ea5e9" : "#f59e0b",
    fontWeight: 700,
    fontSize: "14px",
    flexShrink: 0,
  }),
  externalLabel: {
    fontSize: "10px",
    fontWeight: 600,
    textTransform: "uppercase" as const,
    letterSpacing: "0.05em",
    color: "var(--vocs-color_text4, #999)",
    padding: "1px 6px",
    borderRadius: "3px",
    backgroundColor: "var(--vocs-color_background4, #2a2a2a)",
    marginLeft: "auto",
    flexShrink: 0,
  },
  // VPC box
  vpcBox: {
    borderRadius: "10px",
    overflow: "hidden",
    border: "1px solid #0ea5e940",
  },
  vpcHeader: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "8px 14px",
    backgroundColor: "#0ea5e9",
    color: "#fff",
    fontWeight: 700,
    fontSize: "12px",
  },
  vpcSubtitle: {
    fontSize: "11px",
    fontWeight: 500,
    opacity: 0.85,
  },
  vpcBody: {
    padding: "10px 12px",
    backgroundColor: "var(--vocs-color_background3, #1a191b)",
    display: "flex",
    flexDirection: "column" as const,
    gap: "6px",
  },
  // Service card
  service: (color: string) => ({
    display: "flex",
    alignItems: "center",
    gap: "10px",
    padding: "8px 12px",
    borderRadius: "6px",
    backgroundColor: "var(--vocs-color_background5, #e8e8e8)",
    border: "1px solid var(--vocs-color_border, #444)",
  }),
  serviceDot: (color: string) => ({
    width: "8px",
    height: "8px",
    borderRadius: "50%",
    backgroundColor: color,
    flexShrink: 0,
  }),
  serviceLabel: {
    fontWeight: 600,
    color: "var(--vocs-color_heading, #fff)",
    fontSize: "12px",
  },
  serviceSub: {
    color: "var(--vocs-color_text3, #838383)",
    fontSize: "11px",
  },
  // Arrow between sections
  arrow: {
    display: "flex",
    justifyContent: "center",
    color: "var(--vocs-color_text4, #666)",
    fontSize: "16px",
    padding: "2px 0",
  },
};

// ─── Component ───────────────────────────────────────────────────

export function WhatJustHappened() {
  const inbound = infra.external.filter((e) => e.direction === "in");
  const outbound = infra.external.filter((e) => e.direction === "out");

  return (
    <div style={s.container}>
      {/* Inbound */}
      {inbound.map((ext) => (
        <div key={ext.label} style={s.external(ext.direction)}>
          <span style={s.externalArrow(ext.direction)}>→</span>
          {ext.label}
          <span style={s.externalLabel}>inbound</span>
        </div>
      ))}

      <div style={s.arrow}>↓</div>

      {/* VPC Box */}
      <div style={s.vpcBox}>
        <div style={s.vpcHeader}>
          <span>{infra.title}</span>
          <span style={s.vpcSubtitle}>{infra.subtitle}</span>
        </div>
        <div style={s.vpcBody}>
          {infra.services.map((svc) => (
            <div key={svc.label} style={s.service(svc.color)}>
              <span style={s.serviceDot(svc.color)} />
              <div>
                <div style={s.serviceLabel}>{svc.label}</div>
                <div style={s.serviceSub}>{svc.sub}</div>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div style={s.arrow}>↓</div>

      {/* Outbound */}
      {outbound.map((ext) => (
        <div key={ext.label} style={s.external(ext.direction)}>
          <span style={s.externalArrow(ext.direction)}>→</span>
          {ext.label}
          <span style={s.externalLabel}>outbound</span>
        </div>
      ))}
    </div>
  );
}
