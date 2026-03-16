/**
 * Client-side zip generation using JSZip.
 * Assembles all generated files into a downloadable project scaffold.
 */

import JSZip from "jszip";
import type { BuilderState } from "./configSchema.ts";
import { generateToml } from "./tomlGenerator.ts";
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
  generateMetadata,
  generateReadme,
} from "./tfGenerator.ts";
import { generateRindexerYaml } from "./rindexerGenerator.ts";
import { generateErpcYaml } from "./erpcGenerator.ts";
import { getAbiContent } from "./abiRegistry.ts";

export async function exportZip(state: BuilderState): Promise<void> {
  const zip = new JSZip();
  const root = zip.folder(state.projectName)!;

  // ── evm-cloud.toml ────────────────────────────────────────────────────
  root.file("evm-cloud.toml", generateToml(state));

  // ── Terraform scaffold ────────────────────────────────────────────────
  root.file("versions.tf", generateVersionsTf(state));
  root.file("main.tf", generateMainTf(state));
  root.file("variables.tf", generateVariablesTf(state));
  root.file("outputs.tf", generateOutputsTf());
  root.file("terraform.auto.tfvars", generateTfvarsJson(state));

  // Secrets example (only if there are sensitive vars)
  const secretsExample = generateSecretsExample(state);
  if (secretsExample) {
    root.file("secrets.auto.tfvars.example", secretsExample);
  }

  // Backend config (only if remote state configured)
  const tfBackend = generateTfBackend(state);
  if (tfBackend && state.stateBackend) {
    const filename = `${state.projectName}.${state.stateBackend.backend}.tfbackend`;
    root.file(filename, tfBackend);
  }

  // ── Config files ──────────────────────────────────────────────────────
  const config = root.folder("config")!;

  config.file("rindexer.yaml", generateRindexerYaml(state));
  config.file("erpc.yaml", generateErpcYaml(state));

  // ABIs — all available ABIs bundled (users may reference multiple)
  const abisDir = config.folder("abis")!;
  for (const abi of ["ERC20.json", "ERC721.json", "PoolManager.json", "AaveV3Pool.json", "AaveV4Spoke.json"]) {
    abisDir.file(abi, getAbiContent(abi));
  }

  // ── Project files ─────────────────────────────────────────────────────
  root.file(".gitignore", generateGitignore());
  root.file("Makefile", generateMakefile());
  root.file(".evm-cloud.json", generateMetadata(state));
  root.file("README.md", generateReadme(state));

  // ── Generate blob and trigger download ────────────────────────────────
  const blob = await zip.generateAsync({ type: "blob" });
  downloadBlob(blob, `${state.projectName}.zip`);
}

function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

