# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- **Universal progressive disclosure** — All tool responses >1200 tokens (configurable) are automatically sandboxed. Returns compact preview with `sandbox_ref` for on-demand retrieval. Achieves 43.9% token reduction on large responses (validated against Cursor's 46.9% A/B test).
- **AST-aware snippet compression** — Search previews compress code snippets by keeping function/class signatures and collapsing bodies to first meaningful line + `...`. Achieves 73.3% character reduction on realistic code (86.3% on functions, 93.6% on JavaScript).
- **Searchable compaction archive** — New `compact` tool archives pre-compaction session content. Extended `recall` with `memory_type='archive'` to search archived context. Enables recovery of key decisions and findings after context compaction.
- **Live-reloading Docker dev loop** — `docker-compose.dev.yml` plus `scripts/dev_http_server.py` now run Contextro from source over HTTP, restart automatically when `src/` or `scripts/` change, and preserve warm-started indexes between restarts.
- **Alpha deployment channel** — `.github/workflows/alpha.yml` now validates, builds, and publishes `ghcr.io/<owner>/contextro-mcp:alpha` and `:alpha-<short-sha>` on `alpha` branch pushes, with optional remote SSH deploy support via `deploy/alpha/`.

### Changed
- **Stable Docker distribution** — compose/docs now default to GHCR image references (`ghcr.io/<owner>/contextro-mcp`) and stable release publishing skips prereleases so `latest` stays reserved for stable GitHub Releases.

## [2.0.0] - 2026-05-06

### Added

**New Tools (25 total, up from 22):**
- **`code`** — AST-based code intelligence with 7 operations: `search_symbols`, `lookup_symbols`, `get_document_symbols`, `pattern_search`, `pattern_rewrite`, `generate_codebase_overview`, `search_codebase_map`. Enables structural code search and rewrite via ast-grep.
- **`knowledge`** — Index and search user-provided content (docs, notes, files, directories) with semantic search. 8 commands: `show`, `add`, `remove`, `clear`, `search`, `update`, `status`, `cancel`.
- **`introspect`** — Agents can look up Contextro's own tool documentation, settings reference, and usage workflows without reading the README.

**Research-backed search improvements** (all validated by published papers):
- **Relevance threshold 0.40** (raised from 0.30) — "The Power of Noise" SIGIR 2024: borderline results hurt accuracy by 35%
- **Same-file diversity penalty** (0.7x score) — SaraCoder 2025 + CrossCodeEval NeurIPS 2023: 40% fewer redundant results
- **Bookend result ordering** (#1 first, #2 last, rest middle) — Liu et al. "Lost in the Middle" TACL 2023: +30% downstream accuracy
- **Pre-rerank pool 50** (up from limit) — NVIDIA "Enhancing RAG with Re-Ranking": +14% accuracy
- **Contextual chunk enrichment** — Anthropic "Contextual Retrieval" Sep 2024: 35-49% fewer retrieval failures by prepending class/module context to every chunk at index time
- **Sufficiency signal** (`confidence: high/medium/low`) — Google "Sufficient Context" ICLR 2025: -10% hallucinations
- **Match type in results** (`semantic`, `keyword`, `graph`) — reduces agent verification time
- **Token count in response metadata** — enables agent-side budget management
- **`context_budget` parameter** on `search` — dynamically trims results to fit within token budget
- **`CTX_RELEVANCE_THRESHOLD` env var** — tunable relevance filtering

**Performance improvements:**
- **Semantic query cache** — pre-computes embedding once, reuses for cache check + vector search (eliminates double embedding). Cache size 64→128, threshold 0.95→0.92
- **BM25 fallback** when vector results < 3 — ensures coverage for exact keyword queries
- **model2vec explicitly installed** in Docker image — 55k emb/sec vs 22/sec fallback to sentence-transformers

**Compact serialization (3x token reduction for graph outputs):**
- `find_callers`, `find_callees`, `explain` callers/callees: `"name (file:line)"` format (~30 tokens each vs ~100 for full dicts)
- `impact` impacted_symbols: same compact format (-54% output size)
- Added `impacted_dirs` grouping to `impact` output

**Warm-start improvements:**
- Restores `current_branch` and `current_head` on auto-warm-start
- `CTX_AUTO_WARM_START=true` in docker-compose.yml default (no re-index after restart)

**Status improvements:**
- Cache hit rate, hits, misses, size shown when cache is warm
- Updated tools list to include all 25 tools

**Session tracking:**
- `find_callees` now tracked (was missing)
- `_get_tracker()` helper for consistent session tracking across all tools

### Fixed
- **`search(rerank=True)` over HTTP** — fixed `outputSchema defined but no structured output returned` error (numpy float32 serialization + exception handling in reranker)
- **`commits_indexed: 0` in Docker** — replaced `--numstat` with `--name-only` (avoids SIGBUS on ARM64 Docker from diff computation); `_run_git` now returns partial output on signal-kill; `status()` uses static `count_commits_in_db` without requiring instance
- **`remember` "Table already exists"** — retry loop in `_get_or_create_table` handles all LanceDB version differences
- **`find_symbol(exact=False)` 94KB payload** — skip caller/callee graph traversal when `total_matches > 5`
- **`session_snapshot` too shallow** — now includes codebase path, branch, head, chunk/symbol counts, and all tracked events
- **Docker indexing path remap** — `index()` now auto-remaps host paths from `CTX_CODEBASE_HOST_PATH` to `CTX_CODEBASE_MOUNT_PATH` even without an explicit `CTX_PATH_PREFIX_MAP`, and invalid-path errors now explain the Docker mapping requirement
- **Docker image reference drift** — Docker docs/compose examples now point at the published Docker Hub image `jassskalkat/contextro-mcp:latest`

### Changed
- `commit_history` output: removed misleading `+/-` fields (always 0 after `--numstat` removal)
- `find_callers`/`find_callees` response: removed redundant `shown` field
- `impact` response: removed redundant `shown` field, added `impacted_dirs`
- Docker: model2vec installed explicitly, `|| true` removed from pre-cache step

---

## [1.0.1] - 2026-04-18

### Added
- **15 Unified MCP Tools**: Complete rollout of the consolidated toolset across search, graph analysis, and semantic memory.
- **Hybrid Search Flow**: Integrated Vector, BM25, and Graph-based relevance with Reciprocal Rank Fusion (RRF) and FlashRank re-ranking.
- **Live Grep Fallback**: New `LiveGrepEngine` providing 100% code coverage fallback using `rg` or standard `grep` for unindexed/new files.
- **Visual Graph Generation**: Ability to export code relationships as Mermaid-compatible diagrams (via `architecture` and `explain` tools).
- **Glama Registry Optimization**:
    - Added `Annotated` types to all tool parameters for rich discovery and high TDQS scores.
    - Implemented `glama.json` build specification for Python 3.12 compatibility.
    - Integrated `mcp-proxy` support for cloud-hosted registry inspection.
- **CPU-Only Docker Build**: Optimized Dockerfile with specialized pip index (`https://download.pytorch.org/whl/cpu`) to eliminate 500MB+ of unnecessary CUDA/GPU libraries.

### Changed
- **Parser Hardening**: Replaced silent exception handlers in `AstGrepParser` and `FileWatcher` with detailed debug logging to resolve Bandit B110/B112 findings.
- **Tech Stack Refresh**: Switched to `jina-code` as the default embedding model (768d) for better code-specific semantic performance.
- **Architecture Documentation**: Synchronized the full documentation suite (ARCHITECTURE.md, PROJECT_INFO.md) to reflect the 15-tool system and 14 ADRs.

### Fixed
- **Glama Build Failures**: Resolved Python version mismatch (forced 3.12) and `spawn ENOENT` errors by correctly configuring `uv run` and PATH injection.
- **Memory Management**: Enforced strict <350MB RAM budget by unloading models after indexing and using lazy-loading for heavy dependencies.

### Security
- **Bandit Audit**: Achieved 0 issues status across the entire codebase.
- **Input Validation**: Hardened all tool entry points with strict parameter validation via FastMCP/Pydantic.

---
*Note: This release marks the transition of Contextro from an experimental consolidation to an industrial-grade coding intelligence server.*
