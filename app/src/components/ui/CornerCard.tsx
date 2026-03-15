import type { ReactNode } from "react";

interface Props {
  children: ReactNode;
  accent?: boolean;
  className?: string;
  hover?: boolean;
}

/**
 * Card with corner bracket decorations — exomonk design system.
 * `accent` uses green corners, default uses muted white corners.
 */
export function CornerCard({ children, accent = false, className = "", hover = false }: Props) {
  const cornerColor = accent
    ? "border-[var(--color-accent)]"
    : "border-white/25";

  return (
    <div
      className={`
        relative border border-[var(--color-border)] bg-[var(--color-surface)] p-6
        ${hover ? "transition-all duration-250 hover:border-[var(--color-border-hover)] hover:-translate-y-0.5 hover:shadow-[0_4px_24px_rgba(0,0,0,0.3)]" : ""}
        ${className}
      `}
    >
      {/* Top-left corner */}
      <div className={`absolute top-0 left-0 w-2.5 h-2.5 border-t border-l ${cornerColor}`} />
      {/* Top-right corner */}
      <div className={`absolute top-0 right-0 w-2.5 h-2.5 border-t border-r ${cornerColor}`} />
      {/* Bottom-left corner */}
      <div className={`absolute bottom-0 left-0 w-2.5 h-2.5 border-b border-l ${cornerColor}`} />
      {/* Bottom-right corner */}
      <div className={`absolute bottom-0 right-0 w-2.5 h-2.5 border-b border-r ${cornerColor}`} />

      {children}
    </div>
  );
}
