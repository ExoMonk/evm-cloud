import type { ReactNode } from "react";

const DOCS_URL = "https://evm-cloud.xyz";
const GITHUB_URL = "https://github.com/evm-cloud/evm-cloud";

export function Layout({ children }: { children: ReactNode }) {
  return (
    <div className="min-h-screen flex flex-col">
      {/* Glass nav — sticky with backdrop blur */}
      <header className="sticky top-0 z-50 border-b border-[var(--color-border)] bg-[rgba(12,12,12,0.85)] backdrop-blur-[20px]">
        <nav className="max-w-6xl mx-auto px-5 md:px-16 h-14 flex items-center justify-between">
          <a href="/" className="text-[11px] uppercase tracking-[0.18em] font-medium text-[var(--color-text)]">
            evm-cloud
          </a>
          <div className="flex items-center gap-6">
            <a
              href={DOCS_URL}
              className="text-[11px] uppercase tracking-[0.18em] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors duration-200"
              target="_blank"
              rel="noopener noreferrer"
            >
              docs
            </a>
            <a
              href={`${DOCS_URL}/docs/examples`}
              className="text-[11px] uppercase tracking-[0.18em] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors duration-200"
              target="_blank"
              rel="noopener noreferrer"
            >
              examples
            </a>
            <a
              href="/"
              className="text-[11px] uppercase tracking-[0.18em] text-[var(--color-text)] font-medium"
            >
              builder
            </a>
            <a
              href={GITHUB_URL}
              className="text-[11px] uppercase tracking-[0.18em] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors duration-200"
              target="_blank"
              rel="noopener noreferrer"
            >
              github
            </a>
          </div>
        </nav>
      </header>

      <main className="flex-1">
        {children}
      </main>

      <footer className="border-t border-[var(--color-border)] py-5 px-5 md:px-16">
        <div className="max-w-6xl mx-auto flex items-center justify-between text-[11px] text-[var(--color-text-muted)] tracking-[0.1em]">
          <span>evm-cloud v0.0.1-alpha8 builder</span>
          <span>pure client-side — your config never leaves the browser</span>
        </div>
      </footer>
    </div>
  );
}
