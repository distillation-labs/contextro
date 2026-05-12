# Changelog

All notable changes to this project are tracked here.

## [Unreleased]

## [0.3.0] - 2026-05-12

### Fixed

- **Call graph now populated for Rust code** — `graph_relationships` was always 0 because the parser never extracted call expressions for Rust functions. Added `extract_rust_calls` to the tree-sitter parser, mirroring the existing Python call extractor. `find_callers`, `find_callees`, `impact`, `explain`, `architecture` hub degrees, and `dead_code` accuracy all benefit immediately.
- **`retrieve` now reads from the compaction archive** — `compact` wrote to `CompactionArchive` but `retrieve` was reading from `OutputSandbox` (a different store). Fixed `handle_retrieve` to use the same archive, so `compact`/`retrieve` round-trips work correctly.
- **`remember`/`forget` tags now accept JSON arrays** — tags were parsed with `as_str()` which silently drops JSON arrays. Both `handle_remember` and `handle_forget` now handle `["tag1", "tag2"]` array format as well as the comma-separated string format.
- **`code(list_symbols)` implemented** — previously returned "Unknown code operation". Now returns all symbols (with caller/callee counts) for a given file or directory path.
- **Vector search (`mode=vector` and `mode=hybrid`)** — vector and hybrid search modes were falling through to BM25 only. Added an in-memory `VectorIndex` backed by the potion-base-8M embedding model. Chunks are now embedded at index time; `mode=vector` does cosine similarity search; `mode=hybrid` fuses BM25 and vector results.

## [0.2.0] - 2026-05-11

### Added

- `packages/plugins/` — pre-built, marketplace-ready plugins for Claude Code, GitHub Copilot CLI, OpenAI Codex, and Kiro.
- Claude Code / Copilot CLI plugin: `.claude-plugin/marketplace.json` catalog, `plugin.json` manifest, `.mcp.json` MCP server wiring, `hooks/hooks.json` SessionStart binary check, and full `dev-contextro-mcp` skill bundle.
- Codex plugin: same structure as Claude Code plugin plus `/contextro:setup` command.
- Kiro plugin: `plugin.json` with inline MCP config, skill bundle, and setup guide.
- `shared/` directory with canonical MCP config and hooks shared across all plugin targets.
- `npx @contextro/skills plugin <claude-code|codex|kiro>` command to generate a complete plugin package on demand.

## [0.1.0] - 2026-05-11

### Added

- Single-binary Rust MCP server with 35 tools.
- Pre-built binaries for macOS, Linux, and Windows.
- npm distribution and Docker image for team/server use.
- Publication kit with paper, figures, and benchmark artifacts.
- `docs-maintainer` and other release-facing docs cleanup.

### Changed

- Moved the repo to a Rust-only runtime and removed Python-era docs and skills.
- Consolidated release-facing documentation under `docs/publication/`.
- Added scripts for installs, deployments, and one-by-one commits.

### Fixed

- Removed stale documentation paths, duplicate docs, and outdated launch artifacts.
