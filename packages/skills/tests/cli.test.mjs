import { describe, it, before, after } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, rmSync, readFileSync, writeFileSync } from "node:fs";
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
      assert.match(out, /Available skills \(1\)/);
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

    it("installs the distributed skill to all supported targets", () => {
      const out = run(`install --dir "${tmpDir}"`);
      assert.match(out, /artifact\(s\) updated/);

      for (const platform of [".claude/skills", ".agents/skills", ".github/skills", ".kiro/skills", ".opencode/skills"]) {
        assert.ok(
          existsSync(join(tmpDir, platform, "dev-contextro-mcp", "SKILL.md")),
          `Missing: ${platform}/dev-contextro-mcp/SKILL.md`
        );
      }

      assert.ok(existsSync(join(tmpDir, ".github", "copilot-instructions.md")));
      assert.ok(existsSync(join(tmpDir, "AGENTS.md")));
      assert.ok(existsSync(join(tmpDir, "docs", "contextro-agent-guide.md")));
      assert.ok(existsSync(join(tmpDir, ".claude", "skills", "dev-contextro-mcp", "references", "tool-decision-tree.md")));
      assert.ok(existsSync(join(tmpDir, ".claude", "skills", "dev-contextro-mcp", "evals", "evals.json")));
      assert.ok(existsSync(join(tmpDir, ".agents", "skills", "dev-contextro-mcp", "references", "tool-decision-tree.md")));
      assert.ok(existsSync(join(tmpDir, ".agents", "skills", "dev-contextro-mcp", "evals", "evals.json")));
    });

    it("installs a single skill", () => {
      const dir2 = join(tmpDir, "single");
      mkdirSync(dir2, { recursive: true });
      const out = run(`install dev-contextro-mcp --dir "${dir2}"`);
      assert.match(out, /artifact\(s\) updated/);
      assert.ok(existsSync(join(dir2, ".claude/skills/dev-contextro-mcp/SKILL.md")));
    });

    it("installs to a specific platform only", () => {
      const dir3 = join(tmpDir, "platform");
      mkdirSync(dir3, { recursive: true });
      const out = run(`install --dir "${dir3}" --platform github`);
      assert.match(out, /artifact\(s\) updated/);
      assert.ok(existsSync(join(dir3, ".github/skills/dev-contextro-mcp/SKILL.md")));
      assert.ok(existsSync(join(dir3, ".github/copilot-instructions.md")));
      assert.ok(!existsSync(join(dir3, ".claude/skills/dev-contextro-mcp/SKILL.md")));
      assert.ok(!existsSync(join(dir3, "AGENTS.md")));
    });

    it("installs codex artifacts without installing skill directories", () => {
      const dir4 = join(tmpDir, "codex");
      mkdirSync(dir4, { recursive: true });
      const out = run(`install --dir "${dir4}" --platform codex`);
      assert.match(out, /artifact\(s\) updated/);
      assert.ok(existsSync(join(dir4, "AGENTS.md")));
      assert.ok(existsSync(join(dir4, "docs/contextro-agent-guide.md")));
      assert.ok(!existsSync(join(dir4, ".claude/skills/dev-contextro-mcp/SKILL.md")));
    });

    it("skips existing without --force", () => {
      const dir5 = join(tmpDir, "force");
      mkdirSync(dir5, { recursive: true });
      run(`install --dir "${dir5}"`);
      const out2 = run(`install --dir "${dir5}"`);
      assert.match(out2, /exists/);
    });

    it("overwrites with --force", () => {
      const dir6 = join(tmpDir, "overwrite");
      mkdirSync(dir6, { recursive: true });
      run(`install --dir "${dir6}"`);
      const out = run(`install --dir "${dir6}" --force`);
      assert.match(out, /artifact\(s\) updated/);
    });

    it("does not overwrite a hand-written AGENTS.md without force", () => {
      const dir7 = join(tmpDir, "preserve-agents");
      mkdirSync(dir7, { recursive: true });
      writeFileSync(join(dir7, "AGENTS.md"), "# hand-written\n", "utf8");
      const out = run(`install --dir "${dir7}" --platform codex`);
      assert.match(out, /exists and is not managed/);
      assert.equal(readFileSync(join(dir7, "AGENTS.md"), "utf8"), "# hand-written\n");
    });

    it("falls back when copilot instructions already exist", () => {
      const dir8 = join(tmpDir, "copilot-fallback");
      mkdirSync(join(dir8, ".github"), { recursive: true });
      writeFileSync(join(dir8, ".github/copilot-instructions.md"), "# hand-written\n", "utf8");
      const out = run(`install --dir "${dir8}" --platform github`);
      assert.match(out, /not managed/);
      assert.ok(existsSync(join(dir8, ".github/instructions/contextro.instructions.md")));
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

    it("removes the installed skill from a platform", () => {
      const out = run(`uninstall dev-contextro-mcp --dir "${tmpDir}" --platform claude`);
      assert.match(out, /Removed/);
      assert.ok(!existsSync(join(tmpDir, ".claude/skills/dev-contextro-mcp")));
      assert.ok(existsSync(join(tmpDir, ".github/skills/dev-contextro-mcp/SKILL.md")));
    });

    it("removes generated codex artifacts", () => {
      const dir2 = join(tmpDir, "codex-remove");
      mkdirSync(dir2, { recursive: true });
      run(`install --dir "${dir2}" --platform codex`);
      const out = run(`uninstall dev-contextro-mcp --dir "${dir2}" --platform codex`);
      assert.match(out, /Removed/);
      assert.ok(!existsSync(join(dir2, "AGENTS.md")));
      assert.ok(!existsSync(join(dir2, "docs/contextro-agent-guide.md")));
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
