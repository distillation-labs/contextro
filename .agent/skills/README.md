# Agent Skills — Single Source of Truth

All skill definitions live here. The platform-specific directories are derived copies.

## Source of truth

```
.agent/skills/<skill-name>/
├── SKILL.md          # Canonical skill definition
├── references/       # Reference docs used in skill instructions
└── evals/            # Eval cases for skill quality testing
```

## Platform copies (SKILL.md only)

| Platform       | Directory           | Reads skills from                          |
|----------------|---------------------|--------------------------------------------|
| GitHub Copilot | `.github/skills/`   | `.github/skills/<name>/SKILL.md`           |
| OpenCode       | `.opencode/skills/` | `.opencode/skills/<name>/SKILL.md`         |
| Kiro CLI       | `.kiro/skills/`     | `.kiro/skills/<name>/SKILL.md`             |

Platform directories contain only `SKILL.md`. Evals and references stay here.

## Updating a skill

1. Edit `.agent/skills/<name>/SKILL.md`
2. Copy to all three platform directories:

```bash
for platform in .github/skills .opencode/skills .kiro/skills; do
  cp .agent/skills/<name>/SKILL.md $platform/<name>/SKILL.md
done
```

Or to sync all skills at once:

```bash
for platform in .github/skills .opencode/skills .kiro/skills; do
  for skill in .agent/skills/*/; do
    name=$(basename "$skill")
    cp "$skill/SKILL.md" "$platform/$name/SKILL.md"
  done
done
```

## Skills

| Skill | Purpose |
|---|---|
| `applied-ai-engineer` | Turn research ideas into benchmarked, observable, production-ready systems |
| `autoresearch` | Autonomous metric-driven experiment loops until a breakthrough target is met |
| `breakthrough-researcher` | Deep technical research, hypothesis generation, ranked experiment backlog |
| `dev-contextro-mcp` | Use Contextro MCP for codebase discovery, search, call graphs, git history, memory |
| `fastmcp-server-engineer` | Build or refactor FastMCP servers, tools, resources, middleware, and validation |
| `mcp-protocol-architect` | Design MCP servers around correct protocol primitives and transport choices |
| `python-systems-engineer` | Python typing, dataclasses, exceptions, thread safety, module structure |
| `rust-extension-engineer` | PyO3/maturin Rust extensions for hot paths, FFI boundaries, parity testing |
