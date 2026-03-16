/**
 * Golden file updater — regenerates all golden files from current generators.
 *
 * Run: npx tsx tests/update-golden.ts
 *
 * Commit the updated golden files. The contract test will then enforce
 * that future generator changes produce identical output.
 */

import { mkdirSync, writeFileSync } from "fs";
import { join } from "path";
import { FIXTURES } from "./fixtures.ts";
import { generateToml } from "../src/lib/tomlGenerator.ts";
import {
  generateVersionsTf,
  generateMainTf,
  generateVariablesTf,
  generateOutputsTf,
  generateTfvarsJson,
  generateSecretsExample,
  generateTfBackend,
} from "../src/lib/tfGenerator.ts";
import { generateRindexerYaml } from "../src/lib/rindexerGenerator.ts";
import { generateErpcYaml } from "../src/lib/erpcGenerator.ts";

const GOLDEN_DIR = join(import.meta.dirname!, "golden");

let totalFiles = 0;

for (const fixture of FIXTURES) {
  const dir = join(GOLDEN_DIR, fixture.name);
  mkdirSync(dir, { recursive: true });

  const files: [string, string][] = [
    ["evm-cloud.toml", generateToml(fixture.state)],
    ["main.tf", generateMainTf(fixture.state)],
    ["versions.tf", generateVersionsTf(fixture.state)],
    ["variables.tf", generateVariablesTf(fixture.state)],
    ["outputs.tf", generateOutputsTf()],
    ["terraform.auto.tfvars", generateTfvarsJson(fixture.state)],
    ["rindexer.yaml", generateRindexerYaml(fixture.state)],
    ["erpc.yaml", generateErpcYaml(fixture.state)],
  ];

  const secrets = generateSecretsExample(fixture.state);
  if (secrets) {
    files.push(["secrets.auto.tfvars.example", secrets]);
  }

  const backend = generateTfBackend(fixture.state);
  if (backend) {
    files.push(["backend.tfbackend", backend]);
  }

  for (const [name, content] of files) {
    writeFileSync(join(dir, name), content);
    totalFiles++;
  }

  console.log(`  ${fixture.name}/ (${files.length} files)`);
}

console.log(`\nGenerated ${totalFiles} golden files across ${FIXTURES.length} fixtures.`);
console.log("Commit these files to lock the current generator output.");
