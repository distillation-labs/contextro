# Shared Plugin Content

This directory contains the canonical content shared across all Contextro plugin targets:

- `mcp.json` — MCP server configuration (wires up the `contextro` binary)
- `hooks.json` — Lifecycle hooks (SessionStart binary check)
- `skill/` — Symlinked from `packages/skills/skills/dev-contextro-mcp/`

Each plugin target (claude-code, codex, kiro) references or copies from this shared source.
