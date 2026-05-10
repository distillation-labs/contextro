# @contextro/skills

**Teach your AI coding agent how to use Contextro.**

One command installs agent skills that make Claude, Cursor, Copilot, Kiro, and OpenCode use Contextro MCP for code discovery instead of reading files one by one.

## Install

```bash
npx @contextro/skills install
```

That's it. Skills are installed into your project's standard skill directories:
- `.agent/skills/` (Claude Code)
- `.github/skills/` (GitHub Copilot)
- `.kiro/skills/` (Kiro)
- `.opencode/skills/` (OpenCode)

## What It Does

After installation, your AI agent will:

1. **Search by meaning** instead of reading 5+ files to find one function
2. **Check impact** before renaming or deleting anything
3. **Trace call graphs** instead of grepping for usages
4. **Use 95% fewer tokens** for code discovery tasks

## Commands

```bash
# Install all skills (default)
npx @contextro/skills install

# Install a specific skill
npx @contextro/skills install dev-contextro-mcp

# Install to a specific platform only
npx @contextro/skills install --platform kiro

# List available skills
npx @contextro/skills list

# Show skill details
npx @contextro/skills info dev-contextro-mcp

# Remove a skill
npx @contextro/skills uninstall contextro-quickstart

# Run the MCP vs no-MCP benchmark on your codebase
npx @contextro/skills benchmark --dir /path/to/your/project
```

## Available Skills

| Skill | Description |
|-------|-------------|
| `dev-contextro-mcp` | Full Contextro integration: search, symbols, call graphs, impact analysis, git history, memory, AST rewrite |
| `contextro-quickstart` | Minimal setup: teaches search, find_symbol, explain, impact |

## Options

| Flag | Description |
|------|-------------|
| `--dir <path>` | Target project directory (default: current directory) |
| `--platform <name>` | Install to one platform only: `agent`, `github`, `kiro`, `opencode` |
| `--force` | Overwrite existing skill files |

## Prerequisites

- **Node.js 18+** (for npx)
- **Contextro** installed in your project: `pip install contextro`
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

Skills are markdown files (`SKILL.md`) that teach AI agents specific workflows. When your agent encounters a task that matches the skill's trigger conditions, it follows the skill's instructions instead of its default behavior.

For example, the `dev-contextro-mcp` skill teaches agents to:
- Use `search("query")` instead of reading multiple files
- Run `impact("Symbol")` before any rename or delete
- Use `explain("Symbol")` before editing unfamiliar code
- Prefer `find_callers()` over grep for usage discovery

## Troubleshooting

### Skills not being picked up

Make sure your agent's skill directory matches one of the installed paths:
```bash
ls .agent/skills/     # Claude Code
ls .github/skills/    # GitHub Copilot
ls .kiro/skills/      # Kiro
ls .opencode/skills/  # OpenCode
```

### Contextro not responding

1. Check it's installed: `pip show contextro`
2. Check MCP is configured: verify your agent's MCP config includes `contextro`
3. Index your project first: tell your agent "Index this project"

### Updating skills

```bash
npx @contextro/skills install --force
```

## License

Proprietary - internal Distillation Labs distribution only.
