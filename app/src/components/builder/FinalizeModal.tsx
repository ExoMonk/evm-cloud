import { useState, useMemo, useEffect } from "react";
import type { BuilderState } from "../../lib/configSchema.ts";
import { generateToml } from "../../lib/tomlGenerator.ts";
import { validate } from "../../lib/configValidator.ts";
import { estimateCost } from "../../lib/costData.ts";
import {
  generateVersionsTf,
  generateMainTf,
  generateVariablesTf,
  generateOutputsTf,
  generateTfvarsJson,
  generateSecretsExample,
  generateTfBackend,
  generateGitignore,
  generateMakefile,
  generateReadme,
} from "../../lib/tfGenerator.ts";
import { exportZip } from "../../lib/zipExporter.ts";
import { generateRindexerYaml } from "../../lib/rindexerGenerator.ts";
import { generateErpcYaml } from "../../lib/erpcGenerator.ts";
import { getAbiContent } from "../../lib/abiRegistry.ts";
import { ArchitectureDiagram } from "./ArchitectureDiagram.tsx";
import { PanZoom } from "../ui/PanZoom.tsx";

interface Props {
  state: BuilderState;
  onClose: () => void;
}

interface FileEntry {
  name: string;
  path: string;
  content: string;
  icon: "tf" | "toml" | "json" | "yaml" | "make" | "md" | "git";
}

/**
 * Full-screen finalize modal — file explorer + code viewer + download.
 * Opened from ReviewStep when user clicks "Finalize".
 */
export function FinalizeModal({ state, onClose }: Props) {
  const [activeFile, setActiveFile] = useState(0);
  const [copied, setCopied] = useState(false);
  const [diagramExpanded, setDiagramExpanded] = useState(false);

  // Lock body scroll when modal is open
  useEffect(() => {
    document.body.style.overflow = "hidden";
    return () => { document.body.style.overflow = ""; };
  }, []);

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose]);

  const issues = useMemo(() => validate(state), [state]);
  const cost = useMemo(() => estimateCost(state), [state]);
  const errors = issues.filter((i) => i.severity === "error");

  // Generate all files
  const files = useMemo((): FileEntry[] => {
    const result: FileEntry[] = [
      { name: "evm-cloud.toml", path: "evm-cloud.toml", content: generateToml(state), icon: "toml" },
      { name: "main.tf", path: "main.tf", content: generateMainTf(state), icon: "tf" },
      { name: "versions.tf", path: "versions.tf", content: generateVersionsTf(state), icon: "tf" },
      { name: "variables.tf", path: "variables.tf", content: generateVariablesTf(state), icon: "tf" },
      { name: "outputs.tf", path: "outputs.tf", content: generateOutputsTf(), icon: "tf" },
      { name: "terraform.auto.tfvars", path: "terraform.auto.tfvars", content: generateTfvarsJson(state), icon: "tf" },
    ];

    const secretsExample = generateSecretsExample(state);
    if (secretsExample) {
      result.push({ name: "secrets.auto.tfvars.example", path: "secrets.auto.tfvars.example", content: secretsExample, icon: "tf" });
    }

    const tfBackend = generateTfBackend(state);
    if (tfBackend && state.stateBackend) {
      result.push({
        name: `${state.projectName}.${state.stateBackend.backend}.tfbackend`,
        path: `${state.projectName}.${state.stateBackend.backend}.tfbackend`,
        content: tfBackend,
        icon: "tf",
      });
    }

    // Config files
    result.push(
      { name: "config/rindexer.yaml", path: "config/rindexer.yaml", content: generateRindexerYaml(state), icon: "yaml" },
      { name: "config/erpc.yaml", path: "config/erpc.yaml", content: generateErpcYaml(state), icon: "yaml" },
    );

    // ABI files — all bundled
    for (const abi of ["ERC20.json", "ERC721.json", "PoolManager.json", "AaveV3Pool.json", "AaveV4Spoke.json"]) {
      result.push({
        name: `config/abis/${abi}`,
        path: `config/abis/${abi}`,
        content: getAbiContent(abi),
        icon: "json",
      });
    }

    // Project files
    result.push(
      { name: "Makefile", path: "Makefile", content: generateMakefile(), icon: "make" },
      { name: ".gitignore", path: ".gitignore", content: generateGitignore(), icon: "git" },
      { name: "README.md", path: "README.md", content: generateReadme(state), icon: "md" },
    );

    return result;
  }, [state]);

  const currentFile = files[activeFile];

  const copyFile = async () => {
    if (!currentFile) return;
    await navigator.clipboard.writeText(currentFile.content);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const iconColor = (icon: FileEntry["icon"]): string => {
    switch (icon) {
      case "tf": return "text-purple-400";
      case "toml": return "text-[var(--color-accent)]";
      case "json": return "text-amber-400";
      case "yaml": return "text-blue-400";
      case "make": return "text-[var(--color-text-dim)]";
      case "md": return "text-[var(--color-text-dim)]";
      case "git": return "text-red-400";
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/80 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="relative w-[95vw] h-[90vh] max-w-7xl border border-[var(--color-border)] bg-[var(--color-bg)] flex flex-col">
        {/* Corner decorations */}
        <div className="absolute top-0 left-0 w-3 h-3 border-t border-l border-[var(--color-accent)]" />
        <div className="absolute top-0 right-0 w-3 h-3 border-t border-r border-[var(--color-accent)]" />
        <div className="absolute bottom-0 left-0 w-3 h-3 border-b border-l border-[var(--color-accent)]" />
        <div className="absolute bottom-0 right-0 w-3 h-3 border-b border-r border-[var(--color-accent)]" />

        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-[var(--color-border)]">
          <div className="flex items-center gap-4">
            <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)]">
              // finalize
            </p>
            <span className="text-[13px] text-[var(--color-accent)]">
              {state.projectName}
            </span>
          </div>
          <div className="flex items-center gap-6">
            {/* Cost badge */}
            {state.infraProfile && (
              <span className="text-[11px] text-[var(--color-text-dim)]">
                ~${cost.monthlyMin}–{cost.monthlyMax}/mo
              </span>
            )}
            {/* Validation badge */}
            {errors.length === 0 ? (
              <span className="text-[11px] text-[var(--color-accent)]">● valid</span>
            ) : (
              <span className="text-[11px] text-[var(--color-error)] group/err relative cursor-help">
                ✕ {errors.length} error{errors.length !== 1 ? "s" : ""}
                <span className="absolute top-6 right-0 hidden group-hover/err:block w-72 p-3 border border-[var(--color-error)]/30 bg-[var(--color-bg)] z-20 space-y-1">
                  {errors.map((e, i) => (
                    <span key={i} className="block text-[10px] text-[var(--color-text-dim)]">✕ {e.message}</span>
                  ))}
                </span>
              </span>
            )}
            <button
              onClick={onClose}
              className="text-[11px] uppercase tracking-[0.15em] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
            >
              esc
            </button>
          </div>
        </div>

        {/* Body: file list | code viewer | architecture */}
        <div className="flex flex-1 min-h-0">

          {/* Left: file list */}
          <div className="w-56 border-r border-[var(--color-border)] overflow-y-auto py-3 scrollbar-none shrink-0">
            <p className="px-4 text-[10px] uppercase tracking-[0.2em] text-[var(--color-text-muted)] mb-2">
              {state.projectName}/
            </p>
            {files.map((file, i) => (
              <button
                key={file.path}
                onClick={() => setActiveFile(i)}
                className={`
                  w-full text-left px-4 py-1.5 flex items-center gap-2 transition-colors
                  ${activeFile === i
                    ? "bg-[var(--color-surface-hover)] text-[var(--color-text)]"
                    : "text-[var(--color-text-dim)] hover:bg-[var(--color-surface)] hover:text-[var(--color-text)]"
                  }
                `}
              >
                <span className={`text-[10px] ${iconColor(file.icon)}`}>
                  {file.icon === "tf" ? "TF" : file.icon === "json" ? "{}" : file.icon === "toml" ? "◆" : file.icon === "yaml" ? "Y" : file.icon === "make" ? "M" : file.icon === "md" ? "#" : "·"}
                </span>
                <span className="text-[11px] truncate">{file.name}</span>
              </button>
            ))}
          </div>

          {/* Center: code viewer */}
          <div className="flex-1 flex flex-col min-w-0">
            <div className="flex items-center justify-between px-5 py-2.5 border-b border-[var(--color-border)]">
              <span className="text-[12px] text-[var(--color-accent)]">
                {currentFile?.name}
              </span>
              <button
                onClick={copyFile}
                className="text-[11px] uppercase tracking-[0.1em] text-[var(--color-text-muted)] hover:text-[var(--color-accent)] transition-colors"
              >
                {copied ? "copied" : "copy"}
              </button>
            </div>
            <pre className="flex-1 p-5 text-[12px] leading-relaxed text-[var(--color-text-dim)] overflow-auto">
              {truncatePreview(currentFile?.content ?? "")}
            </pre>
          </div>

          {/* Right: architecture diagram (pannable + zoomable) */}
          <div className="w-[490px] border-l border-[var(--color-border)] bg-[var(--color-surface)] shrink-0 relative flex flex-col">
            <div className="flex items-center justify-between px-3 py-1.5 border-b border-[var(--color-border)]">
              <span className="text-[9px] uppercase tracking-[0.2em] text-[var(--color-text-muted)]">// architecture</span>
              <button
                onClick={() => setDiagramExpanded(true)}
                className="text-[13px] px-2 py-1 text-[var(--color-text-muted)] hover:text-[var(--color-accent)] hover:bg-[var(--color-accent-dim)] transition-colors"
                title="Expand architecture view"
              >
                ↗
              </button>
            </div>
            <PanZoom className="flex-1 flex items-center justify-center">
              <div className="p-4">
                <ArchitectureDiagram state={state} compact />
              </div>
            </PanZoom>
            <div className="px-3 py-1 border-t border-[var(--color-border)]">
              <span className="text-[8px] text-[var(--color-text-muted)]">scroll to zoom · drag to pan · double-click to reset</span>
            </div>
          </div>
        </div>

        {/* Expanded architecture overlay */}
        {diagramExpanded && (
          <div className="absolute inset-0 z-10 bg-[var(--color-bg)] flex flex-col">
            <div className="flex items-center justify-between px-6 py-4 border-b border-[var(--color-border)]">
              <p className="text-[11px] uppercase tracking-[0.2em] text-[var(--color-text-muted)]">
                // architecture
              </p>
              <button
                onClick={() => setDiagramExpanded(false)}
                className="text-[11px] uppercase tracking-[0.15em] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
              >
                ← back to files
              </button>
            </div>
            <PanZoom className="flex-1 flex items-center justify-center">
              <div className="p-8 w-full max-w-4xl">
                <ArchitectureDiagram state={state} />
              </div>
            </PanZoom>
            <div className="px-6 py-2 border-t border-[var(--color-border)]">
              <span className="text-[9px] text-[var(--color-text-muted)]">scroll to zoom · drag to pan · double-click to reset</span>
            </div>
          </div>
        )}

        {/* Footer: download */}
        <div className="flex items-center justify-between px-6 py-4 border-t border-[var(--color-border)]">
          <div className="text-[11px] text-[var(--color-text-muted)]">
            {files.length} files — pure client-side, nothing leaves your browser
          </div>
          <div className="flex items-center gap-4">
            <button
              onClick={onClose}
              className="text-[11px] uppercase tracking-[0.15em] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors px-4 py-2"
            >
              ← back
            </button>
            <button
              disabled={errors.length > 0}
              onClick={() => exportZip(state)}
              className={`
                px-6 py-2.5 text-[11px] uppercase tracking-[0.2em] font-medium border transition-all
                ${errors.length > 0
                  ? "border-[var(--color-border)] text-[var(--color-text-muted)] cursor-not-allowed"
                  : "border-[var(--color-accent)] text-[var(--color-accent)] hover:bg-[var(--color-accent-dim)]"
                }
              `}
            >
              download zip
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

const MAX_PREVIEW_LINES = 60;

function truncatePreview(content: string): string {
  const lines = content.split("\n");
  if (lines.length <= MAX_PREVIEW_LINES) return content;
  return lines.slice(0, MAX_PREVIEW_LINES).join("\n") + `\n\n// ... ${lines.length - MAX_PREVIEW_LINES} more lines (full content in downloaded zip)`;
}
