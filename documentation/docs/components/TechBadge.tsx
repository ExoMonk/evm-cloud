import React from "react";

type Badge = {
  label: string;
  color: string;
  href?: string;
};

const badges: Record<string, Badge> = {
  rindexer: {
    label: "rindexer",
    color: "#e44d26",
    href: "https://rindexer.xyz",
  },
  evm: {
    label: "EVM",
    color: "#627eea",
    href: "https://ethereum.org/developers/docs/evm/",
  },
  terraform: {
    label: "Infrastructure",
    color: "#7b42bc",
    href: "https://www.terraform.io",
  },
  erpc: {
    label: "eRPC",
    color: "#0ea5e9",
    href: "https://erpc.cloud",
  },
  aws: {
    label: "AWS",
    color: "#ff9900",
    href: "https://aws.amazon.com",
  },
  docker: {
    label: "Docker",
    color: "#2496ed",
    href: "https://www.docker.com",
  },
  kubernetes: {
    label: "Kubernetes",
    color: "#326ce5",
    href: "https://kubernetes.io",
  },
  clickhouse: {
    label: "ClickHouse",
    color: "#fadb14",
    href: "https://clickhouse.com",
  },
  postgresql: {
    label: "PostgreSQL",
    color: "#336791",
    href: "https://www.postgresql.org",
  },
  helm: {
    label: "Helm",
    color: "#0f1689",
    href: "https://helm.sh",
  },
};

const style = {
  container: {
    display: "flex",
    flexWrap: "wrap" as const,
    gap: "8px",
    padding: "4px 0",
  },
  badge: (color: string) => ({
    display: "inline-flex",
    alignItems: "center",
    gap: "6px",
    padding: "4px 12px 4px 8px",
    borderRadius: "6px",
    backgroundColor: `${color}18`,
    border: `1px solid ${color}40`,
    color: "var(--vocs-color_heading, #fff)",
    fontSize: "13px",
    fontWeight: 500,
    textDecoration: "none",
    transition: "border-color 0.15s, background-color 0.15s",
  }),
  dot: (color: string) => ({
    width: "8px",
    height: "8px",
    borderRadius: "50%",
    backgroundColor: color,
    flexShrink: 0,
  }),
};

export function TechBadges({ items }: { items: string[] }) {
  return (
    <div style={style.container}>
      {items.map((key) => {
        const badge = badges[key];
        if (!badge) return null;
        const Tag = badge.href ? "a" : "span";
        const props = badge.href
          ? { href: badge.href, target: "_blank", rel: "noopener noreferrer" }
          : {};
        return (
          <Tag key={key} style={style.badge(badge.color)} {...props}>
            <span style={style.dot(badge.color)} />
            {badge.label}
          </Tag>
        );
      })}
    </div>
  );
}
