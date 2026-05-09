#!/usr/bin/env node

/**
 * @contextro/skills CLI
 *
 * Install Contextro agent skills into your project.
 * Usage: npx @contextro/skills [command] [options]
 */

import { existsSync, mkdirSync, cpSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { join, resolve, basename } from "node:path";
import { fileURLToPath } from "node:url";
import { execSync } from "node:child_process";

const __dirname = fileURLToPath(new URL(".", import.meta.url));
const SKILLS_DIR = join(__dirname, "..", "skills");
const VERSION = JSON.parse(readFileSync(join(__dirname, "..", "package.json"), "utf8")).version;

// Platform directories where skills get installed
const PLATFORMS = [
  ".agent/skills",
  ".github/skills",
  ".kiro/skills",
  ".opencode/skills",
];

function usage() {
  console.log(`
@contextro/skills v${VERSION}

Usage: npx @contextro/skills <command> [options]

Commands:
  install [skill...]     Install skills into the current project (default: all)
  list                   List available skills
  info <skill>           Show details about a skill
  uninstall <skill...>   Remove installed skills
  benchmark              Run MCP vs no-MCP experiment (requires contextro)

Options:
  --dir <path>           Target project directory (default: cwd)
  --platform <name>      Install to specific platform only
                         (agent|github|kiro|opencode)
  --force                Overwrite existing skills without prompting
  --help, -h             Show this help
  --version, -v          Show version

Examples:
  npx @contextro/skills install
  npx @contextro/skills install dev-contextro-mcp
  npx @contextro/skills list
  npx @contextro/skills install --platform kiro
`);
}

function getAvailableSkills() {
  if (!existsSync(SKILLS_DIR)) return [];
  return readdirSync(SKILLS_DIR, { withFileTypes: true })
    .filter((d) => d.isDirectory() && existsSync(join(SKILLS_DIR, d.name, "SKILL.md")))
    .map((d) => d.name);
}

function getSkillMetadata(skillName) {
  const skillPath = join(SKILLS_DIR, skillName, "SKILL.md");
  if (!existsSync(skillPath)) return null;
  const content = readFileSync(skillPath, "utf8");
  // Parse YAML frontmatter
  const match = content.match(/^---\n([\s\S]*?)\n---/);
  if (!match) return { name: skillName, description: "" };
  const yaml = match[1];
  const name = yaml.match(/^name:\s*(.+)$/m)?.[1]?.trim() || skillName;
  const desc = yaml.match(/description:\s*>\s*\n([\s\S]*?)(?=\n\w|\n---)/)?.[1]?.trim()
    || yaml.match(/description:\s*(.+)$/m)?.[1]?.trim() || "";
  return { name, description: desc.replace(/\n\s*/g, " ") };
}

function installSkill(skillName, targetDir, platforms, force) {
  const srcDir = join(SKILLS_DIR, skillName);
  if (!existsSync(join(srcDir, "SKILL.md"))) {
    console.error(`  ✗ Skill "${skillName}" not found`);
    return false;
  }

  let installed = 0;
  for (const platform of platforms) {
    const destDir = join(targetDir, platform, skillName);
    const destFile = join(destDir, "SKILL.md");

    if (existsSync(destFile) && !force) {
      console.log(`  ⊘ ${platform}/${skillName} (exists, use --force to overwrite)`);
      continue;
    }

    mkdirSync(destDir, { recursive: true });
    cpSync(join(srcDir, "SKILL.md"), destFile);
    installed++;
  }

  if (installed > 0) {
    console.log(`  ✓ ${skillName} → ${installed} platform(s)`);
  }
  return installed > 0;
}

function uninstallSkill(skillName, targetDir, platforms) {
  let removed = 0;
  for (const platform of platforms) {
    const destDir = join(targetDir, platform, skillName);
    if (existsSync(destDir)) {
      rmSync(destDir, { recursive: true });
      removed++;
    }
  }
  if (removed > 0) {
    console.log(`  ✓ Removed ${skillName} from ${removed} platform(s)`);
  } else {
    console.log(`  ⊘ ${skillName} not installed`);
  }
}

function resolvePlatforms(platformArg) {
  if (!platformArg) return PLATFORMS;
  const map = {
    agent: ".agent/skills",
    github: ".github/skills",
    kiro: ".kiro/skills",
    opencode: ".opencode/skills",
  };
  const p = map[platformArg];
  if (!p) {
    console.error(`Unknown platform: ${platformArg}. Use: agent, github, kiro, opencode`);
    process.exit(1);
  }
  return [p];
}

// --- Main ---

const args = process.argv.slice(2);
if (args.includes("--help") || args.includes("-h") || args.length === 0) {
  usage();
  process.exit(0);
}
if (args.includes("--version") || args.includes("-v")) {
  console.log(VERSION);
  process.exit(0);
}

const command = args[0];
const flags = {};
const positional = [];

for (let i = 1; i < args.length; i++) {
  if (args[i] === "--dir") { flags.dir = args[++i]; }
  else if (args[i] === "--platform") { flags.platform = args[++i]; }
  else if (args[i] === "--force") { flags.force = true; }
  else if (!args[i].startsWith("-")) { positional.push(args[i]); }
}

const targetDir = resolve(flags.dir || process.cwd());
const platforms = resolvePlatforms(flags.platform);

switch (command) {
  case "install": {
    const available = getAvailableSkills();
    const toInstall = positional.length > 0 ? positional : available;
    const invalid = toInstall.filter((s) => !available.includes(s));
    if (invalid.length) {
      console.error(`Unknown skills: ${invalid.join(", ")}`);
      console.error(`Available: ${available.join(", ")}`);
      process.exit(1);
    }
    console.log(`Installing ${toInstall.length} skill(s) into ${targetDir}\n`);
    let count = 0;
    for (const skill of toInstall) {
      if (installSkill(skill, targetDir, platforms, flags.force)) count++;
    }
    console.log(`\n✓ Done. ${count} skill(s) installed.`);
    break;
  }

  case "list": {
    const available = getAvailableSkills();
    if (!available.length) {
      console.log("No skills available.");
      break;
    }
    console.log(`Available skills (${available.length}):\n`);
    for (const name of available) {
      const meta = getSkillMetadata(name);
      console.log(`  ${name}`);
      if (meta?.description) console.log(`    ${meta.description.slice(0, 80)}`);
    }
    break;
  }

  case "info": {
    if (!positional[0]) { console.error("Usage: info <skill>"); process.exit(1); }
    const meta = getSkillMetadata(positional[0]);
    if (!meta) { console.error(`Skill "${positional[0]}" not found`); process.exit(1); }
    console.log(`Skill: ${meta.name}`);
    console.log(`Description: ${meta.description}`);
    const content = readFileSync(join(SKILLS_DIR, positional[0], "SKILL.md"), "utf8");
    console.log(`\n${content}`);
    break;
  }

  case "uninstall": {
    if (!positional.length) { console.error("Usage: uninstall <skill...>"); process.exit(1); }
    for (const skill of positional) {
      uninstallSkill(skill, targetDir, platforms);
    }
    break;
  }

  case "benchmark": {
    console.log("MCP vs no-MCP benchmark runner");
    console.log("See: docs/EXPERIMENT_FRAMEWORK.md for methodology.\n");
    const codebase = flags.dir || process.cwd();
    try {
      execSync(
        `node "${join(__dirname, "experiment.mjs")}" --codebase "${codebase}"`,
        { stdio: "inherit", env: { ...process.env } }
      );
    } catch (e) {
      process.exit(e.status || 1);
    }
    break;
  }

  default:
    console.error(`Unknown command: ${command}`);
    usage();
    process.exit(1);
}
