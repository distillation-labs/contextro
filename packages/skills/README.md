# @contextro/skills

**Distribute the canonical Contextro MCP skill bundle for coding agents.**

This package distributes exactly one user-facing skill bundle: `packages/skills/skills/dev-contextro-mcp/`.

That canonical bundle contains:
- `SKILL.md`
- `references/`
- `evals/`

The installer copies that bundle to the agent surfaces that support skills and writes the smallest repo-local instruction files needed for agents that rely on instructions instead of skills.

This package does not build vendor plugin packages. It installs:
- skill bundles for hosts that support `SKILL.md`
- instruction files for hosts that use repo guidance
- nothing that replaces MCP server setup itself

## Install

```bash
npx @contextro/skills install
```

By default the installer writes these repo-local artifacts:
- `.claude/skills/dev-contextro-mcp/` for Claude Code skills
- `.agents/skills/dev-contextro-mcp/` as the canonical skill bundle path
- `.github/skills/dev-contextro-mcp/` plus `.github/copilot-instructions.md` for GitHub Copilot
- `AGENTS.md` plus `docs/contextro-agent-guide.md` for Codex-style agents
- `.kiro/skills/dev-contextro-mcp/` for Kiro skills
- `.opencode/skills/dev-contextro-mcp/` for OpenCode skills

When you install into a normal project, each skill target receives the full bundle: `SKILL.md`, `references/`, and `evals/`.

## What It Does

After installation, your AI agent will:

1. **Search by meaning** instead of reading 5+ files to find one function
2. **Check impact** before renaming or deleting anything
3. **Trace call graphs** instead of grepping for usages
4. **Use 95% fewer tokens** for code discovery tasks

## Commands

```bash
# Install the distributed skill to all supported targets
npx @contextro/skills install

# Install the skill explicitly
npx @contextro/skills install dev-contextro-mcp

# Install to a specific platform only
npx @contextro/skills install --platform claude
npx @contextro/skills install --platform agents
npx @contextro/skills install --platform github
npx @contextro/skills install --platform codex

# List available skills
npx @contextro/skills list

# Show skill details
npx @contextro/skills info dev-contextro-mcp

# Remove the skill from one platform
npx @contextro/skills uninstall dev-contextro-mcp --platform github

# Run the MCP vs no-MCP benchmark on your codebase
npx @contextro/skills benchmark --dir /path/to/your/project
```

## Available Skills

| Skill | Description |
|-------|-------------|
| `dev-contextro-mcp` | Full Contextro integration: search, symbols, call graphs, impact analysis, git history, memory, AST rewrite |

No other internal Contextro skills are distributed by this package.

## Options

| Flag | Description |
|------|-------------|
| `--dir <path>` | Target project directory (default: current directory) |
| `--platform <name>` | Install to one platform only: `claude`, `agents`, `github`, `codex`, `kiro`, `opencode` |
| `--force` | Overwrite existing skill files |

## Prerequisites

- **Node.js 18+** (for npx)
- **Contextro** installed and runnable as the `contextro` binary
- **MCP configured** in your agent (see the root `README.md` for connection examples)

## Benchmark

Run a controlled experiment comparing MCP-augmented vs baseline agent performance:

```bash
npx @contextro/skills benchmark --dir /path/to/your/codebase
```

This runs identical tasks in two arms:
- **Control**: agent uses grep + file reads (baseline)
- **Treatment**: agent uses Contextro MCP tools

Output is written to `./experiment_results/`:
- `config.json` — experiment configuration
- `results.json` — per-task metrics for both arms
- `summary.json` — aggregated comparison

### Typical Results

| Metric | Without MCP | With MCP | Improvement |
|--------|-------------|----------|-------------|
| Tokens per task | ~1,700 | ~50 | **95% reduction** |
| Tool calls | 4 | 1 | **70% reduction** |
| Files read | 3 | 0 | **100% reduction** |

## How Skills Work

The distributed skill is a markdown bundle (`SKILL.md` plus `references/` and `evals/`) that teaches AI agents how to use Contextro for repository discovery.

Concept split:
- `skill bundle`: reusable workflow content loaded by hosts that support `SKILL.md`
- `instruction file`: repo guidance such as `.github/copilot-instructions.md` or `AGENTS.md`
- `MCP server`: the actual Contextro tool/runtime integration
- `plugin`: a vendor-specific packaging surface; this package does not generate plugins

`dev-contextro-mcp` teaches agents to:
- Use `search("query")` instead of reading multiple files
- Run `impact("Symbol")` before any rename or delete
- Use `explain("Symbol")` before editing unfamiliar code
- Prefer `find_callers()` over grep for usage discovery

## Platform Mapping

| Target | Surface type | Installed artifacts |
|--------|--------------|---------------------|
| Claude Code | Skill bundle | `.claude/skills/dev-contextro-mcp/` |
| Repo skills | Skill bundle | `.agents/skills/dev-contextro-mcp/` |
| GitHub Copilot | Skill bundle + instructions | `.github/skills/dev-contextro-mcp/`, `.github/copilot-instructions.md`, fallback `.github/instructions/contextro.instructions.md` |
| Codex-style agents | Instructions | `AGENTS.md`, `docs/contextro-agent-guide.md` |
| Kiro | Skill bundle | `.kiro/skills/dev-contextro-mcp/` |
| OpenCode | Skill bundle | `.opencode/skills/dev-contextro-mcp/` |

## Troubleshooting

### Skills not being picked up

Make sure the generated files match the target agent you care about:
```bash
ls .claude/skills/dev-contextro-mcp
ls .agents/skills/dev-contextro-mcp
ls .github/skills/dev-contextro-mcp
ls .github/copilot-instructions.md
ls AGENTS.md
ls .kiro/skills/dev-contextro-mcp
ls .opencode/skills/dev-contextro-mcp
```

If you already have a hand-written `.github/copilot-instructions.md` or `AGENTS.md`, the installer will not overwrite it unless you pass `--force`. In that case it falls back to a namespaced Copilot instruction file and leaves your existing top-level docs untouched.

### Contextro not responding

1. Check the binary is available: `command -v contextro`
2. Check MCP is configured: verify your agent's MCP config includes `contextro`
3. Index your project first: tell your agent "Index this project"

### Updating skills

```bash
npx @contextro/skills install --force
```

In this repo, `.agents/skills/dev-contextro-mcp/` is development-only Contextro context. The packaged copy under `packages/skills/skills/dev-contextro-mcp/` is the distribution source that ships to end users.

## License

Source-available under the Business Source License 1.1 (`BUSL-1.1`).
Internal production use is permitted under the Additional Use Grant in
`LICENSE`. This version converts to Apache License 2.0 on 2030-05-11, or on
the fourth anniversary of its first public release, whichever comes first.
