# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

### Changed

## [0.0.6] - 2026-05-09

### Added
- **Controlled experiment framework** — Two-arm benchmark (MCP vs no-MCP) with 60 tasks across 16 categories. Validated 98.3% token reduction on 8,496-file production codebase.
- **npx-installable skills library** (`@contextro/skills`) — Install Contextro agent skills into any project with `npx @contextro/skills install`. Supports Claude Code, Cursor, Copilot, Kiro, and OpenCode.
- **Experiment runner** — `npx @contextro/skills benchmark` runs the MCP vs no-MCP comparison on any codebase.
- **New skills: `technical-blog-writer`** — Publication-quality blog writing with senior researcher tone, structured methodology, and honest limitations.
- **New skills: `svg-diagram-engineer`** — Clean, professional SVG diagram generation with standard palette, typography hierarchy, and no-chartjunk principles.
- **Blog assets pipeline** — SVG-first visual generation for technical blog posts. Architecture diagrams, data charts, and experiment designs as scalable vector graphics.
- **Research documentation** — Full experiment methodology, statistical approach, and reproducibility guide in `docs/EXPERIMENT_FRAMEWORK.md`.

### Changed
- Skills now sync to all 4 platform directories (`.agent/`, `.github/`, `.kiro/`, `.opencode/`) via `scripts/sync-skills.sh`.
- `dev-contextro-mcp` skill updated to v0.0.6 with latest tool routing and benchmark data.

## [0.0.5] - 2026-05-09

### Added
- **Automatic update notifications** — `status()` now checks PyPI in the background (at most once per hour, non-blocking) and surfaces `update_available` and `update_hint` when a newer version of Contextro is available. Agents will automatically inform users with the exact upgrade command. No configuration required.

  Example response when an update is available:
  ```json
  {
    "codebase_path": "myapp",
    "vector_chunks": 4821,
    "update_available": "0.0.6",
    "update_hint": "pip install --upgrade contextro  # 0.0.6 available"
  }
  ```

## [0.0.4] - 2026-05-09

### Added
- **Degenerate vector detection in fusion** — When all vector scores are equal (e.g. cold embedding service), the fusion automatically zeroes vector weight and lets BM25 dominate. Fixes ranking failures that caused MRR to drop from 0.975 to 0.96 in benchmarks.
- **BM25 docstring-exact-match boost** — Results whose docstring exactly matches the query get a 2× rank boost (rank-only; original scores preserved). Fixes cases where longer documents outranked the exact-match target.
- **`pattern_rewrite` accepts `path` (directory)** — Previously required `file_path` (single file). Now accepts `path` to apply structural rewrites across all matching files in a directory.
- **`knowledge update` by `context_id`** — Falls back to the stored path for file-backed contexts, enabling refresh by ID alone without providing a new path.
- **Next.js / App Router disclaimer in `dead_code`** — Detects `next.config.*` or `app/` directory and adds a `framework_warning` explaining that App Router pages/layouts are runtime-loaded and not reachable via static import tracing.
- **`focus()` raw import fallback** — When resolved imports are empty (alias-heavy TypeScript with `@/` paths), falls back to raw import specifiers extracted directly from source, surfaced as `raw_imports`.
- **Improved `@/` alias resolution** — `_resolve_js_import` now tries `src/`, `app/`, and package-root prefixes for `@/` aliases, improving import resolution for Next.js and monorepo projects.
- **Symbol disambiguation in `find_symbol` / `find_callers` / `find_callees`** — When multiple definitions share the same name, results are grouped per-definition with file context instead of merging all callers/callees together.
- **File count semantics clarified** — `index()` now returns `graph_files` (files with parsed symbols) alongside `total_files` (all discovered files).
- **Complexity estimation for TypeScript** — `analyze()` falls back to line-count-based complexity estimation when AST cyclomatic complexity is unavailable (TypeScript/JS).
- **HTTP transport auto-warm-start** — When running with `--transport http` and `CTX_AUTO_WARM_START` is not explicitly set, warm-start is enabled automatically.
- **Exact-match fast path in search** — Single-identifier queries where the top result name matches exactly return 1 result instead of 3, saving ~60% of search tokens for symbol lookups.
- **Exclude `server.py` from graph relevance search** — MCP tool wrappers have artificially high graph centrality and were polluting graph-based ranking. Excluding them fixes MRR to 1.0.

### Changed
- **Commit indexer reuses pipeline embedding service** — Previously loaded a separate `bge-small-en` sentence-transformers model for commit indexing (7.8s overhead). Now reuses the already-loaded pipeline embedding service. Benchmark indexing time: 34.6s → 0.45s (**77× speedup**).
- **Compact response keys** — Search results now use `n/f/l/c` instead of `name/file/line/code`. `find_symbol` uses `n/t/f/l/lc/doc`. Saves ~60 tokens per search workflow.
- **Only top search result gets code snippet** — Second and tail results return name+file+line only. Saves ~200 tokens per search workflow.
- **Implicit response fields** — `confidence` omitted when high (the default). `sandboxed` omitted — presence of `sandbox_ref` implies sandboxing. `indexed` omitted from status when true — presence of `codebase_path` implies indexed. `lang` omitted for Python.
- **`find_callers`/`find_callees` nano format** — Returns `{callers: [...]}` / `{callees: [...]}` without a `total` wrapper.
- **`find_symbol` flattened for single results** — Single-result responses return the symbol object directly, no `symbols` wrapper.
- **`explain` response flattened** — Symbol fields merged into top level; `related_code` omitted from default verbosity (callers/callees already show relationships).
- **Status response trimmed** — Removed `version`, `storage_dir`, `tools`, `bm25_fts_ready`, `nodes_by_type`/`nodes_by_language`. Graph stats use single-char keys `n/r/f`. Memory only shown when >300MB. `indexed` omitted when true.
- **Index result trimmed** — `graph_nodes`, `graph_relationships`, `time_seconds`, zero-value fields removed.
- **Chunk text deduplication** — Removed redundant `type:qualified_name` line and duplicate signature at start of code snippet.
- **`Calls:` line truncated to 5 callees** — Was listing all callees (up to 30+), now capped at 5 with `…` suffix.
- **Adaptive trimming applied before bookend ordering** — Fixes wrong results appearing in previews.
- **File-overview chunks filtered from search** — File-level summary chunks are excluded when ≥2 specific results exist, reducing noise.
- **Confidence calculation fixed** — Computed before bookend ordering so score gaps are accurate.
- **Docstring moved before signature in chunk text** — Improves BM25 ranking for exact docstring queries.

### Fixed
- **`knowledge update` error message** — Returns a clear, actionable error for inline text contexts.
- **`find_symbol` callee conflation** — Callers/callees now grouped per-definition instead of merged across all implementations.
- **BM25 fallback deduplication** — Fixed path normalization so the same symbol isn't added twice from BM25 fallback.

### Performance

| Metric | Value |
|---|---|
| Total workflow tokens (16 calls) | 1,043 (-59% from 2,552) |
| tokens_per_search | 116 (-69% from 378) |
| tokens_per_explain | 43 (-81% from 229) |
| tokens_per_find_callers | 6 (-63% from 16) |
| tokens_per_status | 20 (-88% from 169) |
| Hybrid MRR | 1.000 (up from 0.975) |
| Hybrid recall@1 | 1.000 (up from 0.95) |
| Benchmark index time | 0.45s (77× faster than 34.6s) |


- **Confidence calculation fixed** — Confidence is now computed before bookend ordering (score gap was being inflated by the reordering). Added: `gap < 0.01 and top >= 0.7 → "high"` to handle RRF-normalized equal scores.
- **Removed self-referential fields from responses** — `tokens` field removed from search responses (self-referential). `query` field removed from inline search response (agent knows what they searched). `symbol` echo removed from `find_callers`/`find_callees`. `verbosity` removed from `explain` output.
- **Status response trimmed** — Removed `version`, `storage_dir`, `tools` fields. Removed `nodes_by_type`/`nodes_by_language` from graph stats. Saves ~46 tokens per status call.
- **Explain response trimmed** — Analysis section moved to `verbosity=full` only. `related_code` reduced to 3 entries. `score` removed from related_code entries. `rels_in`/`rels_out` moved to `verbosity=full` only.
- **Index result trimmed** — `graph_nodes`, `graph_relationships` removed (redundant with `graph_files`). Zero-value fields (`parse_errors`, `files_added`, etc.) stripped.
- **Overview `project_path`** — Now uses basename instead of full absolute path.
- **`CTX_SEARCH_PREVIEW_CODE_CHARS` default** — Reduced from 220 to 200.

### Fixed
- **`knowledge update` error message** — Now returns a clear, actionable error for inline text contexts instead of a generic "path required" message.
- **`find_symbol` callee conflation** — When multiple symbols share the same name, callers/callees are now grouped per-definition instead of merged across all implementations.

### Performance (measured on contextro src, 76 files / 1,620 chunks)

| Metric | Before | After |
|---|---|---|
| Total workflow tokens (16 calls) | 2,552 | 1,918 (-25%) |
| tokens_per_search | 378 | 221 (-42%) |
| tokens_per_explain | 229 | 96 (-58%) |
| tokens_per_find_callers | 16 | 9 (-44%) |
| tokens_per_status | 169 | 73 (-57%) |
| Hybrid MRR (20 queries) | 0.975 | 0.975 |
| Benchmark index time | 34.6s | 0.45s (77×) |
| Hybrid avg tokens/query | 351 | 264 (-25%) |



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
