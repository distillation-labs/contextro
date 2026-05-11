# @contextro/plugins

Pre-built Contextro plugins for AI coding agents. One-command install that wires up the MCP server, skill bundle, and lifecycle hooks.

## Supported Agents

| Agent | Directory | Install Method |
|---|---|---|
| Claude Code + Copilot CLI | `claude-code/` | `/plugin marketplace add` |
| OpenAI Codex | `codex/` | `/plugin marketplace add` |
| Kiro | `kiro/` | Copy to `.kiro/skills/` |

## Claude Code / GitHub Copilot CLI

Both use the same plugin format. Add the marketplace and install:

```
/plugin marketplace add distillation-labs/contextro-plugins
/plugin install contextro@distillation-labs-contextro
/reload-plugins
```

Or test locally:

```
claude --plugin-dir ./packages/plugins/claude-code/plugins/contextro
```

## OpenAI Codex

Codex adopted the Claude Code plugin standard. Same flow:

```
/plugin marketplace add distillation-labs/contextro-plugins-codex
/plugin install contextro@distillation-labs-contextro-codex
/reload-plugins
```

## Kiro

Copy the plugin into your project:

```bash
cp -r packages/plugins/kiro/ .kiro/plugins/contextro/
```

Or use the skills CLI:

```bash
npx @contextro/skills install --platform kiro
```

## What Each Plugin Contains

| Component | Purpose |
|---|---|
| `.mcp.json` | Wires up the `contextro` binary as an MCP server |
| `skills/dev-contextro-mcp/` | Teaches the agent how to use all 35 Contextro tools |
| `hooks/hooks.json` | SessionStart check that warns if binary is missing |
| `commands/setup.md` | (Codex only) `/contextro:setup` command |

## Generate Plugins Programmatically

The `@contextro/skills` CLI can generate a complete plugin package:

```bash
npx @contextro/skills plugin claude-code --dir ./output
npx @contextro/skills plugin codex --dir ./output
npx @contextro/skills plugin kiro --dir ./output
```

## Prerequisites

- **Contextro binary** installed and in PATH: `npm install -g contextro`
- The plugin wires up MCP automatically — no manual JSON editing needed

## Architecture

```
packages/plugins/
├── shared/              # Canonical MCP config, hooks, skill symlink
├── claude-code/         # Claude Code + Copilot CLI marketplace
│   ├── .claude-plugin/
│   │   └── marketplace.json
│   └── plugins/contextro/
│       ├── .claude-plugin/plugin.json
│       ├── .mcp.json
│       ├── hooks/hooks.json
│       └── skills/dev-contextro-mcp/
├── codex/               # OpenAI Codex marketplace
│   ├── .claude-plugin/
│   │   └── marketplace.json
│   └── plugins/contextro/
│       ├── .claude-plugin/plugin.json
│       ├── .mcp.json
│       ├── hooks/hooks.json
│       ├── commands/setup.md
│       └── skills/dev-contextro-mcp/
└── kiro/                # Kiro plugin
    ├── plugin.json
    ├── guides/setup.md
    └── skills/dev-contextro-mcp/
```

## Relationship to @contextro/skills

- `@contextro/skills` installs skill bundles into existing projects (repo-local files)
- `@contextro/plugins` provides marketplace-ready plugin packages (installable via `/plugin`)

They're complementary: skills for teams setting up a project, plugins for individual users who want zero-friction onboarding.

## License

Source-available under the Business Source License 1.1 (`BUSL-1.1`).
