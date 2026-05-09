import { describe, it, before, after } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, rmSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { execSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";

const __dirname = fileURLToPath(new URL(".", import.meta.url));
const CLI = join(__dirname, "..", "bin", "cli.mjs");
const run = (args) => execSync(`node "${CLI}" ${args}`, { encoding: "utf8" });

describe("@contextro/skills CLI", () => {
  describe("--help", () => {
    it("prints usage", () => {
      const out = run("--help");
      assert.match(out, /@contextro\/skills/);
      assert.match(out, /install/);
      assert.match(out, /list/);
      assert.match(out, /benchmark/);
    });
  });

  describe("--version", () => {
    it("prints version", () => {
      const out = run("--version").trim();
      assert.match(out, /^\d+\.\d+\.\d+$/);
    });
  });

  describe("list", () => {
    it("lists available skills", () => {
      const out = run("list");
      assert.match(out, /dev-contextro-mcp/);
      assert.match(out, /contextro-quickstart/);
      assert.match(out, /Available skills \(2\)/);
    });
  });

  describe("info", () => {
    it("shows skill details", () => {
      const out = run("info dev-contextro-mcp");
      assert.match(out, /Contextro MCP/);
      assert.match(out, /search/);
    });

    it("errors on unknown skill", () => {
      assert.throws(() => run("info nonexistent"), /not found/);
    });
  });

  describe("install", () => {
    let tmpDir;

    before(() => {
      tmpDir = join(tmpdir(), `contextro-skills-test-${Date.now()}`);
      mkdirSync(tmpDir, { recursive: true });
    });

    after(() => {
      rmSync(tmpDir, { recursive: true, force: true });
    });

    it("installs all skills to all platforms", () => {
      const out = run(`install --dir "${tmpDir}"`);
      assert.match(out, /2 skill\(s\) installed/);

      // Verify files exist in all platforms
      for (const platform of [".agent/skills", ".github/skills", ".kiro/skills", ".opencode/skills"]) {
        assert.ok(
          existsSync(join(tmpDir, platform, "dev-contextro-mcp", "SKILL.md")),
          `Missing: ${platform}/dev-contextro-mcp/SKILL.md`
        );
        assert.ok(
          existsSync(join(tmpDir, platform, "contextro-quickstart", "SKILL.md")),
          `Missing: ${platform}/contextro-quickstart/SKILL.md`
        );
      }
    });

    it("installs a single skill", () => {
      const dir2 = join(tmpDir, "single");
      mkdirSync(dir2, { recursive: true });
      const out = run(`install dev-contextro-mcp --dir "${dir2}"`);
      assert.match(out, /1 skill\(s\) installed/);
      assert.ok(existsSync(join(dir2, ".agent/skills/dev-contextro-mcp/SKILL.md")));
      assert.ok(!existsSync(join(dir2, ".agent/skills/contextro-quickstart/SKILL.md")));
    });

    it("installs to a specific platform only", () => {
      const dir3 = join(tmpDir, "platform");
      mkdirSync(dir3, { recursive: true });
      const out = run(`install --dir "${dir3}" --platform kiro`);
      assert.match(out, /installed/);
      assert.ok(existsSync(join(dir3, ".kiro/skills/dev-contextro-mcp/SKILL.md")));
      assert.ok(!existsSync(join(dir3, ".agent/skills/dev-contextro-mcp/SKILL.md")));
    });

    it("skips existing without --force", () => {
      const dir4 = join(tmpDir, "force");
      mkdirSync(dir4, { recursive: true });
      run(`install --dir "${dir4}"`);
      const out2 = run(`install --dir "${dir4}"`);
      assert.match(out2, /exists/);
    });

    it("overwrites with --force", () => {
      const dir5 = join(tmpDir, "overwrite");
      mkdirSync(dir5, { recursive: true });
      run(`install --dir "${dir5}"`);
      const out = run(`install --dir "${dir5}" --force`);
      assert.match(out, /installed/);
    });

    it("errors on unknown skill name", () => {
      assert.throws(() => run(`install nonexistent --dir "${tmpDir}"`), /Unknown skills/);
    });
  });

  describe("uninstall", () => {
    let tmpDir;

    before(() => {
      tmpDir = join(tmpdir(), `contextro-skills-uninstall-${Date.now()}`);
      mkdirSync(tmpDir, { recursive: true });
      run(`install --dir "${tmpDir}"`);
    });

    after(() => {
      rmSync(tmpDir, { recursive: true, force: true });
    });

    it("removes an installed skill", () => {
      const out = run(`uninstall contextro-quickstart --dir "${tmpDir}"`);
      assert.match(out, /Removed/);
      assert.ok(!existsSync(join(tmpDir, ".agent/skills/contextro-quickstart")));
    });
  });
});

describe("experiment runner", () => {
  const EXPERIMENT = join(__dirname, "..", "bin", "experiment.mjs");
  let tmpDir;

  before(() => {
    tmpDir = join(tmpdir(), `contextro-experiment-${Date.now()}`);
    mkdirSync(tmpDir, { recursive: true });
  });

  after(() => {
    rmSync(tmpDir, { recursive: true, force: true });
  });

  it("runs and produces output files", () => {
    const codebase = join(__dirname, "..");
    const output = join(tmpDir, "results");
    execSync(
      `node "${EXPERIMENT}" --codebase "${codebase}" --output "${output}"`,
      { encoding: "utf8" }
    );

    assert.ok(existsSync(join(output, "config.json")));
    assert.ok(existsSync(join(output, "results.json")));
    assert.ok(existsSync(join(output, "summary.json")));

    const summary = JSON.parse(readFileSync(join(output, "summary.json"), "utf8"));
    assert.equal(summary.tasks_run, 6);
    assert.ok(summary.improvement.token_reduction_pct > 0);
    assert.ok(summary.mcp.total_tokens < summary.control.total_tokens);
  });

  it("summary shows MCP is more efficient", () => {
    const codebase = join(__dirname, "..");
    const output = join(tmpDir, "results2");
    execSync(
      `node "${EXPERIMENT}" --codebase "${codebase}" --output "${output}"`,
      { encoding: "utf8" }
    );

    const summary = JSON.parse(readFileSync(join(output, "summary.json"), "utf8"));
    assert.ok(summary.improvement.token_reduction_pct >= 80, "Expected >=80% token reduction");
    assert.ok(summary.improvement.files_read_reduction_pct === 100, "MCP should read 0 files");
  });
});
