# Changelog

All notable changes to this project are tracked here.

## [Unreleased]

## [0.6.0] - 2026-05-12

### Fixed

- **`code(lookup_symbols)` now accepts JSON arrays** — previously the `symbols` parameter was parsed with `as_str()`, so passing `symbols: ["A","B"]` always returned "Missing required parameter: symbols". Now accepts both a JSON array `["A","B"]` and a comma-separated string `"A,B"`.
- **`tags` tool restored** — the tool was silently removed in a prior refactor. It is now re-added and returns all unique tags across all stored memories, sorted alphabetically. `MemoryStore::list_tags()` is the new backing method.
- **`find_callers` / `find_callees` now hint on type nodes** — when the queried symbol is a struct, class, or enum (which have no call-graph edges by definition), the response includes a `hint` field explaining that types have no call edges and suggesting querying a method or constructor instead. Previously the tool silently returned 0 with no explanation.
- **`recall` now finds semantically related memories** — switched from AND matching (all query words must appear) to OR matching (any stemmed word matches), with Rust-side re-ranking by match count. Stop words (`how`, `does`, `work`, `use`, `is`, etc.) are filtered before matching. Word stems are used (`indexing` → `index`, `storing` → `stor`) so paraphrases no longer silently miss relevant memories.
- **`explain` now populates `docstring` for Rust functions** — the parser previously left `docstring` as `null` for all Rust symbols. It now scans the lines immediately before each function/struct definition for `///` doc comments and populates the field.

## [0.5.0] - 2026-05-12

### Fixed

- **Vector search fully operational** — the embedding model (`potion-code-16M`) was never loading in any prior version because `Model2Vec::from_pretrained` expects a local filesystem path, not a HuggingFace model ID string. Added `find_hf_cache_path()` which resolves the HuggingFace Hub local cache layout (`~/.cache/huggingface/hub/models--{owner}--{name}/snapshots/{hash}/`) and passes the correct directory path to the model loader. Vector search now reports `vector_chunks` equal to the full chunk count after indexing (e.g. 400+ for a Rust workspace), and `mode=hybrid` and `mode=vector` return real semantic results with confidence scores. This was the root cause of all vector/semantic search issues reported since v0.1.0.
- **Server no longer hangs under concurrent requests** — v0.4.0 added a `parking_lot::Mutex` dispatch lock to serialize tool calls and fix a recall race condition, but `parking_lot::Mutex::lock()` is a blocking call that starves Tokio worker threads when used inside async tasks. Multiple simultaneous MCP requests would cause the server to deadlock after the first response. The blocking mutex has been removed; concurrent request handling now works correctly.

## [0.4.0] - 2026-05-12

### Fixed

- **Embedding model upgraded to potion-base-16M** — switched from `minishlab/potion-base-8M` to `minishlab/potion-base-16M` for higher quality code embeddings. Model load failures now log at `error` level so they are never silently swallowed.
- **`explain` resolves to the most connected symbol** — when multiple symbols share a name, `resolve_symbol` now ranks candidates by call frequency (in_degree + out_degree) before selecting. `explain("search")` now returns the main search function with 6 callers instead of a 1-line error constructor with 0 connections.
- **`circular_dependencies` no longer produces false positives** — replaced the call-edge-based file graph with an import-edge scanner that reads `use crate::` and `use super::` statements from Rust source files. Normal cross-module function calls no longer inflate the cycle count.
- **`test_coverage_map` now detects inline Rust test modules** — files containing `#[cfg(test)]` or `#[test]` attributes are now counted as covered regardless of filename. Coverage percentage is no longer stuck at 0% for codebases that rely on inline `#[test]` blocks.
- **Generic names filtered from `architecture` and `analyze` rankings** — `new`, `len`, `get`, `is_empty`, `clone`, `default`, and other stdlib-common method names are now excluded from hub symbol and high-connectivity rankings, making the results much more architecturally meaningful.
- **`introspect` now supports multi-word queries** — query terms are split on whitespace and each word must appear in the tool name or description. Queries like "semantic search" or "index codebase" now return correct results instead of 0 matches.
- **`knowledge(add)` reports accurate chunk counts** — the response now reflects the actual number of chunks stored (not a formula that could over-report). Empty content returns an error instead of falsely claiming 1 chunk was indexed, which was causing `knowledge(search)` to return 0 results after a no-op add.

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
