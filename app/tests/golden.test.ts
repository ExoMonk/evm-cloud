/**
 * Schema contract test (M1) — Golden file comparison.
 *
 * Tests that TypeScript generators produce consistent output across changes.
 * Golden files are committed snapshots. When generators change intentionally,
 * run `npm run test:update-golden` to regenerate them.
 *
 * Future: Rust CLI generates the same golden files → this test catches drift.
 */

import { describe, it, expect } from "vitest";
import { readFileSync, existsSync } from "fs";
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

const GOLDEN_DIR = join(__dirname, "golden");

interface GeneratedFile {
  name: string;
  content: string;
}

function generateAllFiles(state: typeof FIXTURES[0]["state"]): GeneratedFile[] {
  const files: GeneratedFile[] = [
    { name: "evm-cloud.toml", content: generateToml(state) },
    { name: "main.tf", content: generateMainTf(state) },
    { name: "versions.tf", content: generateVersionsTf(state) },
    { name: "variables.tf", content: generateVariablesTf(state) },
    { name: "outputs.tf", content: generateOutputsTf() },
    { name: "terraform.auto.tfvars", content: generateTfvarsJson(state) },
    { name: "rindexer.yaml", content: generateRindexerYaml(state) },
    { name: "erpc.yaml", content: generateErpcYaml(state) },
  ];

  const secrets = generateSecretsExample(state);
  if (secrets) {
    files.push({ name: "secrets.auto.tfvars.example", content: secrets });
  }

  const backend = generateTfBackend(state);
  if (backend) {
    files.push({ name: "backend.tfbackend", content: backend });
  }

  return files;
}

describe("Golden file contract tests", () => {
  for (const fixture of FIXTURES) {
    describe(fixture.name, () => {
      const files = generateAllFiles(fixture.state);
      const fixtureDir = join(GOLDEN_DIR, fixture.name);

      for (const file of files) {
        it(`${file.name} matches golden file`, () => {
          const goldenPath = join(fixtureDir, file.name);

          if (!existsSync(goldenPath)) {
            // First run — no golden file yet. Skip with a message.
            // Run `npm run test:update-golden` to generate them.
            console.warn(
              `Golden file missing: ${goldenPath}\n` +
              `Run 'npm run test:update-golden' to generate golden files.`
            );
            return;
          }

          const golden = readFileSync(goldenPath, "utf-8");
          expect(file.content).toBe(golden);
        });
      }
    });
  }
});

describe("Generator sanity checks", () => {
  it("all fixtures produce non-empty TOML", () => {
    for (const fixture of FIXTURES) {
      const toml = generateToml(fixture.state);
      expect(toml.length).toBeGreaterThan(50);
      expect(toml).toContain("[project]");
      expect(toml).toContain(fixture.state.projectName);
    }
  });

  it("all fixtures produce valid main.tf with module block", () => {
    for (const fixture of FIXTURES) {
      const mainTf = generateMainTf(fixture.state);
      expect(mainTf).toContain('module "evm_cloud"');
      expect(mainTf).toContain("source");
      expect(mainTf).toContain("var.project_name");
    }
  });

  it("all fixtures produce variables.tf with project_name", () => {
    for (const fixture of FIXTURES) {
      const varsTf = generateVariablesTf(fixture.state);
      expect(varsTf).toContain('variable "project_name"');
    }
  });

  it("AWS fixtures include aws_region, bare metal fixtures do not", () => {
    for (const fixture of FIXTURES) {
      const tfvars = generateTfvarsJson(fixture.state);
      if (fixture.state.provider === "aws") {
        expect(tfvars).toContain("aws_region");
      } else {
        expect(tfvars).not.toContain("aws_region");
      }
    }
  });

  it("ClickHouse fixtures use clickhouse storage, Postgres uses postgres", () => {
    for (const fixture of FIXTURES) {
      const rindexer = generateRindexerYaml(fixture.state);
      if (fixture.state.databaseProfile.includes("clickhouse")) {
        expect(rindexer).toContain("clickhouse:");
        expect(rindexer).not.toContain("postgres:");
      } else {
        expect(rindexer).toContain("postgres:");
        expect(rindexer).not.toContain("clickhouse:");
      }
    }
  });

  it("eRPC config includes all selected chains", () => {
    for (const fixture of FIXTURES) {
      const erpc = generateErpcYaml(fixture.state);
      for (const chain of fixture.state.chains) {
        const chainId = chain === "ethereum" ? 1 : chain === "polygon" ? 137 : 0;
        if (chainId > 0) {
          expect(erpc).toContain(`chainId: ${chainId}`);
        }
      }
      // Always includes public endpoint fallback
      expect(erpc).toContain("repository://evm-public-endpoints.erpc.cloud");
    }
  });

  it("monitoring variables only appear for k8s engines with monitoring enabled", () => {
    for (const fixture of FIXTURES) {
      const varsTf = generateVariablesTf(fixture.state);
      const isK8s = fixture.state.computeEngine === "k3s" || fixture.state.computeEngine === "eks";
      const hasMonitoring = fixture.state.monitoring?.enabled;

      if (isK8s && hasMonitoring) {
        expect(varsTf).toContain("monitoring_enabled");
      } else if (!isK8s) {
        expect(varsTf).not.toContain("monitoring_enabled");
      }
    }
  });
});
