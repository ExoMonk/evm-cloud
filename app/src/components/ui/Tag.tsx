/**
 * Outline-only tag pill — exomonk design system.
 */
export function Tag({ label, active = false }: { label: string; active?: boolean }) {
  return (
    <span
      className={`
        inline-block text-[11px] px-2 py-0.5 border transition-colors
        ${active
          ? "border-[var(--color-accent)] text-[var(--color-accent)]"
          : "border-[var(--color-border)] text-[var(--color-text-muted)] hover:border-[var(--color-border-hover)] hover:text-[var(--color-text-dim)]"
        }
      `}
    >
      {label}
    </span>
  );
}
