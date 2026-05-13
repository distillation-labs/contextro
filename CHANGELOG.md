# Changelog

All notable changes to this project are tracked here.

## [Unreleased]

## [0.8.1] - 2026-05-12

### Fixed

- `pattern_search` and `pattern_rewrite` now use the `ignore` crate walker (respects `.gitignore`, unlimited depth) instead of a hand-rolled 3-level walker — previously missed all files more than 3 directories deep in monorepos.
- `focus` on a directory now returns all symbols grouped by file instead of returning 0 results.
- `knowledge/search` now uses word-level scoring as a fallback when exact substring matching fails, so queries like "how is the frontend structured" match relevant indexed content.
- `architecture` and `dead_code` no longer surface JS/TS test framework globals (`describe`, `test`, `it`, `expect`, `beforeEach`, etc.) or common noise identifiers (`export`, `await`, `id`, `name`, `type`) as architectural hubs or dead code candidates.

## [0.8.0] - 2026-05-12

### Fixed

- JS/TS parser now extracts call relationships from function bodies — `find_callers`, `find_callees`, `impact`, and `explain` are functional on TypeScript and JavaScript codebases.
- JS/TS parser now recognises arrow functions (`const foo = () =>`), `export const`, `export default`, abstract classes, and class methods in addition to bare `function`/`class` declarations.
- Rust parser now tracks `impl` blocks and extracts methods with correct `parent` context; previously all methods inside `impl` were silently dropped.
- Rust parser now extracts call relationships from function and method bodies.
- Rust parser now extracts enums and traits as symbols.
- Docstrings populated for JS/TS (JSDoc `/** */` and `//` comments) and Rust (`///` doc comments), improving `explain` and search quality.
- `code_snippet` populated for JS/TS and Rust symbols, making `lookup_symbols` with `include_source` work without falling back to file reads.

## [0.7.0] - 2026-05-12

### Fixed

- **Backward compatibility for renamed parameters** — agents that used v0.4.0 parameter names no longer silently fail. All four graph tools (`find_callers`, `find_callees`, `explain`, `impact`) now accept both `symbol_name` (current) and `symbol` (v0.4.0). The `code` tool accepts both `operation` and `action`. `code(search_symbols)` accepts both `symbol_name` and `query`. `code(list_symbols)` routes correctly based on presence of `file_path`. `forget` accepts both `memory_id` (current) and `ids` array (v0.4.0). `knowledge` accepts a bare `query` without `command` and defaults to search.
- **`code(search_codebase_map)` returns symbol map** — previously returned a raw directory listing. Now returns a per-file breakdown of symbols with name, type, line number, and caller/callee counts. Accepts an optional `query` to filter by symbol name and `path` to restrict to a subdirectory.
- **Vector search confidence scores now match BM25** — in standalone `mode=vector` queries, raw cosine similarity (e.g. 0.146) was displayed while BM25 showed 1.0 for every top result. Vector results are now normalized so the top result always shows 1.0, making scores comparable across modes.
- **`impact` explains entry points** — when a symbol has 0 transitive callers (i.e. nothing in the AST calls it), the response now includes a `hint` explaining it is a root entry point and suggesting manual checks for external callers.
- **Knowledge base auto-populated on index** — after a successful `index()`, Contextro scans for `README.md`, `CLAUDE.md`, `AGENTS.md`, `CONTRIBUTING.md`, and `docs/index.md` in the indexed root and automatically adds them to the knowledge store. Subsequent `knowledge(search)` queries work on a fresh install without requiring manual `knowledge(add)`. The KB is only populated once (skipped if already has content).
- **Tool descriptions updated** — all 35 tool definitions now include explicit parameter names, types, and descriptions in their JSON schemas. Agents reading `list_tools` will see the correct parameter names without needing to rely on external documentation.

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
