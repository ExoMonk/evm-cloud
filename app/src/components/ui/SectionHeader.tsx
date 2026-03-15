/**
 * Comment-style section header: // SECTION_NAME
 * exomonk design system.
 */
export function SectionHeader({ label }: { label: string }) {
  return (
    <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-4">
      // {label}
    </p>
  );
}
