# Contextro Rust Rewrite — Phase-Based Implementation Plan

**Total Python to port:** ~8,600 lines across 40+ modules  
**Target:** Single compiled Rust binary, zero Python dependencies  
**Estimated crate count:** 8 workspace crates + 1 binary

---

## Phase 1: Foundation (Core Types + Config + Parsing)

**Goal:** Build the type system, configuration, and code parsing layer. No search, no MCP server yet. Just: "given a file, produce symbols."

### 1.1 — `contextro-core` crate

Port: `core/models.py` (210 lines), `core/graph_models.py` (220 lines), `core/interfaces.py` (45 lines), `core/exceptions.py` (65 lines)

- [ ] Define `SymbolType` enum (Function, Class, Method, Variable)
- [ ] Define `Symbol` struct (frozen, all fields from Python dataclass)
- [ ] Define `ParsedFile` struct with `is_successful()`, `symbol_count()`, `get_symbols_by_type()`
- [ ] Define `CodebaseIndex` struct
- [ ] Define `MemoryType` enum and `Memory` struct
- [ ] Define `NodeType` enum (16 variants)
- [ ] Define `RelationshipType` enum (10 variants)
- [ ] Define `UniversalLocation`, `UniversalNode`, `UniversalRelationship` structs
- [ ] Define `UniversalGraph` with HashMap-based storage + secondary indexes
- [ ] Define `IParser` and `IEngine` traits (Rust trait equivalents of ABCs)
- [ ] Define error types: `ContextroError` enum with variants for Parse, Indexing, Embedding, Search, Config, Graph, Memory, Fusion, Auth, RateLimit
- [ ] Implement `From<>` conversions for error propagation
- [ ] Add serde Serialize/Deserialize derives on all public types
- [ ] Write unit tests for all type constructors and validation logic

### 1.2 — `contextro-config` crate

Port: `config.py` (230 lines)

- [ ] Define `Settings` struct with all 70+ fields, grouped by category
- [ ] Implement `Settings::from_env()` — reads `CTX_*` environment variables with type coercion
- [ ] Implement `project_storage_dir(codebase_path)` — SHA256 slug generation
- [ ] Implement `get_settings()` singleton via `OnceLock<Settings>`
- [ ] Implement `reset_settings()` for testing
- [ ] Write tests for env var parsing, defaults, and type coercion edge cases

### 1.3 — `contextro-parsing` crate

Port: `parsing/treesitter_parser.py` (310 lines), `parsing/astgrep_parser.py` (310 lines), `parsing/language_registry.py` (195 lines), `parsing/file_watcher.py` (165 lines)

- [ ] Define `EXTENSION_MAP` — 40+ extensions → language names
- [ ] Define `TREESITTER_LANGUAGES` and `ASTGREP_LANGUAGES` sets
- [ ] Define `LanguageConfig` per-language AST node type configs
- [ ] Implement `get_language_for_file()`, `get_supported_extensions()`
- [ ] Implement `TreeSitterParser` using `tree-sitter` crate
  - [ ] `can_parse(filepath)` — extension check
  - [ ] `parse_file(filepath)` → `ParsedFile` with symbols, imports, calls
  - [ ] `_extract_symbols()` — AST traversal for classes, functions, methods, constants
  - [ ] `_extract_imports()` — import statement extraction
  - [ ] `_extract_calls_from_node()` — call site extraction
  - [ ] `_extract_docstring()` — docstring extraction from function/class bodies
  - [ ] Binary file detection
- [ ] Implement `AstGrepParser` using `ast-grep-core` crate
  - [ ] `parse_file()` — extract functions, classes, imports, calls into `UniversalGraph`
  - [ ] `parse_directory()` — batch parse all supported files
  - [ ] CALLS edge creation with deduplication and builtin filtering
- [ ] Implement `DebouncedFileWatcher` using `notify` crate (Rust equivalent of watchdog)
  - [ ] Debounced callbacks
  - [ ] File filtering (skip temp, hidden, unsupported)
  - [ ] `consume_recent_changes()` for polling mode
- [ ] Support top 15 languages initially: Python, JavaScript, TypeScript, Rust, Go, Java, C, C++, Ruby, PHP, C#, Kotlin, Swift, Scala, Lua
- [ ] Write integration tests: parse real files, verify symbol extraction matches Python output

### Phase 1 Validation

- [ ] Parse the Contextro Python codebase itself with the Rust parser
- [ ] Compare symbol output against Python parser output (same symbols, same line numbers)
- [ ] Benchmark: parsing speed vs Python tree-sitter (expect 3–5x faster)

---

## Phase 2: Indexing Pipeline

**Goal:** Given a directory, produce a fully indexed codebase: chunks in LanceDB, symbols in graph, BM25 index ready.

### 2.1 — File Operations (internal modules in `contextro-indexing`)

Port: `indexing/file_discovery.py` (80 lines), existing `ctx_fast` Rust code

- [ ] Implement `discover_files()` using `ignore` crate (already done in ctx_fast, move to internal module)
- [ ] Implement `scan_mtimes()` using `rayon` parallel stat
- [ ] Implement `hash_files()` using `xxhash-rust`
- [ ] Implement `diff_mtimes()` for incremental change detection
- [ ] Implement `stat_files()` for batch file metadata
- [ ] Git operations via `git2`: `current_branch()`, `head_hash()`, `is_repo()`, `changed_files()`, `status()`

### 2.2 — Embedding Service

Port: `indexing/embedding_service.py` (310 lines)

- [ ] Implement `EmbeddingService` trait with `embed()` and `embed_batch()`
- [ ] Implement `Model2VecEmbedder` using `model2vec` crate
  - [ ] Load potion-code-16m from HuggingFace Hub or local path
  - [ ] Single text embedding
  - [ ] Batch embedding
  - [ ] Model unloading
- [ ] Implement `FastEmbedEmbedder` using `fastembed` crate (for transformer models)
  - [ ] Support jina-code, bge-small-en, nomic-embed
  - [ ] ONNX inference via `ort`
- [ ] Implement LRU embedding cache (256 entries)
- [ ] Implement `get_embedding_service()` singleton
- [ ] Write benchmarks: embeddings/sec for potion-code-16m (target: ≥55k/sec)

### 2.3 — Chunker

Port: `indexing/chunker.py` (130 lines), `indexing/smart_chunker.py` (175 lines), `indexing/chunk_context.py` (95 lines)

- [ ] Define `CodeChunk` struct (id, text, filepath, symbol_name, symbol_type, language, line_start, line_end, signature, parent, docstring, vector)
- [ ] Implement `generate_chunk_id()` — SHA-256 deterministic ID
- [ ] Implement `create_chunk_text(symbol)` — Anthropic "Contextual Retrieval" format
- [ ] Implement `build_symbol_context_header()` — rich/minimal mode
- [ ] Implement `normalize_chunk_path()`, `module_hint_from_path()`
- [ ] Implement `create_relationship_chunks()` — caller→callee context chunks
- [ ] Implement `create_file_context_chunks()` — module overview chunks
- [ ] Write tests: verify chunk text format matches Python output

### 2.4 — Indexing Pipeline

Port: `indexing/pipeline.py` (480 lines), `indexing/parallel_indexer.py` (115 lines)

- [ ] Implement `IndexingPipeline` struct
  - [ ] `index(codebase_path)` — full index: discover → parse → chunk → embed → store
  - [ ] `incremental_index(codebase_path)` — content-hash change detection, only re-index changed files
  - [ ] `multi_index(codebase_paths)` — multi-repo indexing with deduplication
- [ ] Parallel file parsing using `rayon` (no GIL limitation — use all cores)
- [ ] Streaming batch processing to limit peak RAM
- [ ] Post-pass call edge building for cross-file resolution
- [ ] Graph persistence to SQLite after indexing
- [ ] Metadata persistence (JSON) with content-hash fingerprints
- [ ] Write integration tests: index a real repo, verify chunk count and graph stats

### Phase 2 Validation

- [ ] Index the Contextro codebase itself
- [ ] Compare: same number of chunks, same symbols in graph, same file fingerprints
- [ ] Benchmark: indexing speed (target: ≥2x faster than Python's 6.8s for 3,349 files)
- [ ] Benchmark: incremental reindex (target: ≤10ms when no changes)
- [ ] Memory usage during indexing (target: <200MB peak)

---

## Phase 3: Search Engines

**Goal:** All three search engines (vector, BM25, graph) working, plus fusion, reranking, and caching.

### 3.1 — Vector Engine

Port: `engines/vector_engine.py` (165 lines)

- [ ] Implement `LanceDBVectorEngine` using native `lancedb` Rust crate
  - [ ] `add(chunks)` — batch insert with PyArrow schema
  - [ ] `search(query_embedding, limit, filters)` — vector similarity search
  - [ ] `delete(ids)` / `delete_by_filepath(filepath)`
  - [ ] `upsert(chunks)` — merge-insert
  - [ ] `count()` / `clear()` / `validate()`
- [ ] Thread-safe with `RwLock` (concurrent reads, exclusive writes)
- [ ] Score conversion: `1 / (1 + distance)`
- [ ] Write tests: add chunks, search, verify results

### 3.2 — BM25 Engine

Port: `engines/bm25_engine.py` (130 lines)

- [ ] Implement `TantivyBM25Engine` using `tantivy` crate
  - [ ] Create schema: id, text, filepath, symbol_name, symbol_type, language, line_start, line_end
  - [ ] `index_chunks(chunks)` — build inverted index
  - [ ] `search(query, limit, filters)` — BM25 search with field filtering
  - [ ] Score normalization to [0, 1]
  - [ ] Incremental index updates (add/delete documents)
- [ ] Write tests: index chunks, search by keyword, verify BM25 scoring

### 3.3 — Graph Engine

Port: `engines/graph_engine.py` (320 lines)

- [ ] Implement `CodeGraph` using `petgraph::DiGraph`
  - [ ] `add_node(node)` — with secondary indexes (by type, language, file, name, name tokens)
  - [ ] `add_relationship(rel)` — directed edge
  - [ ] `get_node(id)` / `find_nodes_by_name(name, exact)` — exact + fuzzy (token-based)
  - [ ] `get_callers(id)` / `get_callees(id)` — direct CALLS traversal
  - [ ] `get_transitive_callers(id, max_depth)` — BFS impact analysis
  - [ ] `remove_file_nodes(filepath)` — incremental reindex support
  - [ ] `get_strongly_connected_components(rel_type)` — Tarjan's SCC
  - [ ] `get_statistics()` — node/edge/file counts
  - [ ] `get_reachable_nodes(start_ids)` — transitive closure
- [ ] Thread-safe with `RwLock`
- [ ] CamelCase/snake_case tokenizer for fuzzy name search
- [ ] Write tests: build graph, traverse callers/callees, verify SCC detection

### 3.4 — Fusion

Port: `engines/fusion.py` (210 lines)

- [ ] Implement `graph_relevance_search()` — query tokenization → graph node matching → centrality scoring
- [ ] Implement `ReciprocalRankFusion`
  - [ ] `fuse(ranked_lists)` — RRF with k=60
  - [ ] `_score_entropy()` — Shannon entropy of score distribution
  - [ ] `_adaptive_weights()` — entropy-based weight adaptation, degenerate retriever detection
  - [ ] Score normalization to [0, 1]
  - [ ] Track fusion sources per result
- [ ] Write tests: verify fusion output matches Python implementation on same inputs

### 3.5 — Reranker

Port: `engines/reranker.py` (120 lines)

- [ ] Implement `Reranker` using `fastembed` cross-encoder support
  - [ ] Lazy model loading
  - [ ] `rerank(query, results, limit)` — cross-encoder reranking
  - [ ] Graceful fallback to passthrough when model unavailable
  - [ ] Model unloading
- [ ] Write tests: verify reranking changes result order appropriately

### 3.6 — Query Cache

Port: `engines/query_cache.py` (185 lines)

- [ ] Implement `QueryCache` with LRU eviction
  - [ ] Exact string match (fast path)
  - [ ] Semantic similarity match (cosine > 0.95 threshold)
  - [ ] Semantic bucketing pre-filter (top-4 content tokens)
  - [ ] TTL-based expiry
  - [ ] `hit_rate` tracking
- [ ] Write tests: cache hit/miss, semantic similarity matching, TTL expiry

### 3.7 — Output Sandbox

Port: `engines/output_sandbox.py` (130 lines)

- [ ] Implement `OutputSandbox`
  - [ ] `store(content)` → reference ID (`sx_` + 8-char MD5)
  - [ ] `retrieve(ref_id, query)` — full or query-filtered retrieval
  - [ ] LRU eviction, TTL expiry
- [ ] Write tests: store/retrieve cycle, query filtering

### 3.8 — Live Grep

Port: `engines/live_grep.py` (145 lines)

- [ ] Implement `LiveGrepEngine`
  - [ ] Detect `rg` (ripgrep) on PATH
  - [ ] `search(query, limit)` — run `rg --json`, parse results
  - [ ] Fallback to `grep -rnI`
  - [ ] Timeout handling (5s default)
- [ ] Write tests: search real files, verify results

### Phase 3 Validation

- [ ] Run the existing 20-query benchmark suite against Rust engines
- [ ] Verify MRR ≥ 1.000 (same as Python)
- [ ] Benchmark: search latency (target: <1ms warm, vs Python's <2ms)
- [ ] Benchmark: fusion overhead (target: <0.1ms)
- [ ] Verify query cache hit rate ≥ 20%

---

## Phase 4: Execution Layer + Memory + Git

**Goal:** The search orchestrator, response formatting, memory system, and git integration.

### 4.1 — Search Execution

Port: `execution/search.py` (575 lines), `execution/compaction.py` (349 lines), `execution/ast_compression.py` (187 lines), `execution/response_policy.py` (384 lines)

- [ ] Implement `classify_query()` — symbol/natural/hybrid routing
- [ ] Implement `SearchExecutionEngine`
  - [ ] `execute(options)` — full pipeline: cache → embed → parallel engine queries → fuse → rerank → filter → diversity → dedup → adaptive trim → sandbox
  - [ ] Relevance threshold filtering (drop < 40% of top score)
  - [ ] Same-file diversity penalty (0.7×)
  - [ ] Overlapping line range deduplication
  - [ ] Adaptive result count (high/medium/low confidence)
  - [ ] Bookend ordering (2nd-best last)
  - [ ] Confidence calculation
- [ ] Implement `SearchResultCompactor`
  - [ ] Strip metadata, shorten keys (symbol_name→n, filepath→f, line_start→l)
  - [ ] Progressive code budgets (top=320, second=220, tail=80 chars)
  - [ ] Query-focal windowing (sliding window scoring)
- [ ] Implement `compress_snippet()` — tree-sitter AST compression (collapse function bodies)
- [ ] Implement `ToolResponsePolicy` — universal progressive disclosure
- [ ] Implement `SearchResponsePolicy` — search-specific sandboxing
- [ ] Write tests: end-to-end search with compaction, verify output format

### 4.2 — Response Formatting

Port: `formatting/response_builder.py` (113 lines), `formatting/token_budget.py` (52 lines), `formatting/toon_encoder.py` (81 lines), `token_counting.py` (40 lines)

- [ ] Implement `TokenBudget` — verbosity-based budgets (summary=500, detailed=2000, full=8000)
- [ ] Implement `estimate_tokens(text)` — word-count approximation (chars/4)
- [ ] Implement `toon_encode()` — TOON format (no-quote keys, T/F booleans, semicolons)
- [ ] Implement `ResponseBuilder` — verbosity-aware response assembly
- [ ] Write tests: token estimation, TOON encoding, budget enforcement

### 4.3 — Memory System

Port: `memory/memory_store.py` (260 lines), `memory/compaction_archive.py` (240 lines), `memory/session_tracker.py` (175 lines)

- [ ] Implement `MemoryStore` using LanceDB
  - [ ] `remember()` — embed + store
  - [ ] `recall()` — semantic search with type/tag/project filters
  - [ ] `forget()` — delete by ID, tags, type, or date
  - [ ] `expire_ttl()` — TTL enforcement
- [ ] Implement `CompactionArchive`
  - [ ] `archive()` — store pre-compaction content, return reference ID
  - [ ] `search()` — semantic + substring search
  - [ ] `retrieve()` — full content retrieval
  - [ ] JSON file persistence with atomic writes
- [ ] Implement `SessionTracker`
  - [ ] Track tool invocations (search, explain, impact, edit lifecycle)
  - [ ] `get_snapshot()` — priority-tiered compressed snapshot
- [ ] Write tests: remember/recall cycle, TTL expiry, archive search

### 4.4 — Git Integration

Port: `git/commit_indexer.py` (340 lines), `git/branch_watcher.py` (210 lines), `git/cross_repo.py` (170 lines)

- [ ] Implement `CommitHistoryIndexer` using `git2` crate
  - [ ] `extract_commits()` — walk commit history via libgit2 (no subprocess)
  - [ ] `index_commits()` — embed commit messages, store in LanceDB
  - [ ] `search_commits()` — semantic search over commit history
- [ ] Implement `RealtimeIndexManager`
  - [ ] Background HEAD polling thread (tokio task)
  - [ ] Branch switch detection
  - [ ] Debounced reindex triggers
  - [ ] File watcher integration
- [ ] Implement `CrossRepoManager`
  - [ ] `register_repo()` / `unregister_repo()`
  - [ ] Unified status across repos
- [ ] Write tests: commit extraction, branch detection, cross-repo registration

### Phase 4 Validation

- [ ] End-to-end search: index → search → compacted response (verify format matches Python)
- [ ] Memory: remember → recall → forget cycle
- [ ] Git: index commits → search commits
- [ ] Benchmark: full search pipeline latency (target: <1ms)

---

## Phase 5: Analysis + Reports + Editing

**Goal:** All analysis tools, report generation, sidecar export, and edit planning.

### 5.1 — Analysis

Port: `analysis/static_analysis.py` (230 lines), `analysis/code_analyzer.py` (210 lines), `analysis/repository_map.py` (530 lines)

- [ ] Implement `RepositoryMap` — file-level repo map
  - [ ] Python/JS import resolution
  - [ ] Entry point detection (package.json, pyproject.toml, filename patterns)
  - [ ] `neighbors()`, `reachable_paths()`, `related_tests()`, `top_degree_files()`
  - [ ] `layer_hint()` — architecture layer inference
- [ ] Implement `analyze_dead_code()` — unreachable files, uncalled private symbols
- [ ] Implement `analyze_circular_dependencies()` — Tarjan SCC at file level
- [ ] Implement `analyze_test_coverage_map()` — static coverage analysis
- [ ] Implement `CodeAnalyzer`
  - [ ] `detect_code_smells()` — long functions, complex functions, large classes
  - [ ] `analyze_complexity()` — complexity distribution
  - [ ] `calculate_quality_metrics()` — maintainability index, quality score
- [ ] Write tests: dead code detection, SCC detection, coverage map accuracy

### 5.2 — Reports + Artifacts

Port: `reports/product.py` (580 lines), `artifacts/sidecars.py` (195 lines), `artifacts/graph_workflow.py` (210 lines)

- [ ] Implement report builders:
  - [ ] `build_sidecar_report()` — per-file `.graph.*` content
  - [ ] `build_focus_report()` — low-token context slice
  - [ ] `build_restore_report()` — project re-entry summary
  - [ ] `build_audit_report()` — prioritized recommendations
  - [ ] `build_docs_sections()` — full docs bundle (9 markdown files)
- [ ] Implement sidecar export/clean with language-aware comment prefixes
- [ ] Implement graph workflow init + watch mode
- [ ] Write tests: verify report content structure

### 5.3 — Editing

Port: `editing/planner.py` (280 lines), `editing/rewrite.py` (250 lines)

- [ ] Implement `build_edit_plan()`
  - [ ] Scope resolution (file/directory/multi-file)
  - [ ] Symbol matching (exact + fuzzy via graph)
  - [ ] Pattern matching via ast-grep
  - [ ] Impact computation (transitive callers)
  - [ ] Related test discovery
  - [ ] Confidence scoring + risk identification
- [ ] Implement `execute_pattern_rewrite()`
  - [ ] ast-grep pattern matching + replacement
  - [ ] Dry-run mode: unified diff generation
  - [ ] Apply mode: file writes
  - [ ] Preview signature for guarded apply (SHA1 + TTL)
  - [ ] Changed symbol tracking
- [ ] Write tests: edit plan generation, pattern rewrite dry-run, diff output

### Phase 5 Validation

- [ ] Run analysis on real codebase, verify dead code / SCC / coverage results
- [ ] Generate sidecars, verify format matches Python output
- [ ] Edit plan + rewrite dry-run, verify diff output

---

## Phase 6: MCP Server + All 35 Tools

**Goal:** Wire everything into the MCP server. Full feature parity.

### 6.1 — Server Infrastructure

Port: `server.py` (4,121 lines — server factory + helpers), `cli/runtime.py`, `state.py` (230 lines)

- [ ] Implement `AppState` — runtime state (replaces Python `SessionState`)
  - [ ] Engine references (vector, BM25, graph, memory)
  - [ ] Git state (commit indexer, branch watcher, cross-repo)
  - [ ] Warm-start from persisted data
  - [ ] Graceful shutdown
- [ ] Implement MCP server using `rmcp` crate
  - [ ] Stdio transport (default, local use)
  - [ ] HTTP transport via `axum` (Docker/team use)
  - [ ] Transport selection via `CTX_TRANSPORT` env var
- [ ] Implement path validation helpers (codebase_path resolution, relative path handling)
- [ ] Implement background version check (optional, non-blocking)

### 6.2 — Tool Implementations (all 35)

Each tool is a `#[tool]` annotated async function on the server struct.

**Discovery & Search tools:**
- [ ] `index(path, paths)` — trigger full/incremental/multi indexing
- [ ] `search(query, limit, language, symbol_type, mode, rerank, live_grep, context_budget)`
- [ ] `find_symbol(name, exact)`
- [ ] `find_callers(symbol_name)`
- [ ] `find_callees(symbol_name)`
- [ ] `explain(symbol_name, verbosity)`
- [ ] `impact(symbol_name, max_depth)`

**Structure & Analysis tools:**
- [ ] `overview()`
- [ ] `architecture()`
- [ ] `analyze(path)`
- [ ] `focus(path, include_code)`
- [ ] `dead_code()`
- [ ] `circular_dependencies()`
- [ ] `test_coverage_map()`

**AST & Code tools:**
- [ ] `code(operation, ...)` — dispatch to: get_document_symbols, search_symbols, lookup_symbols, pattern_search, pattern_rewrite, edit_plan, search_codebase_map

**Git tools:**
- [ ] `commit_history(path, limit, since)`
- [ ] `commit_search(query, limit, branch, author)`

**Memory tools:**
- [ ] `remember(content, memory_type, tags, ttl)`
- [ ] `recall(query, limit, memory_type, tags)`
- [ ] `forget(memory_id, tags, memory_type)`
- [ ] `knowledge(command, ...)` — add/search/show/remove/update

**Session tools:**
- [ ] `compact(content)`
- [ ] `session_snapshot()`
- [ ] `restore()`

**Multi-repo tools:**
- [ ] `repo_add(path, name, index_now)`
- [ ] `repo_remove(path, name)`
- [ ] `repo_status()`

**Artifact tools:**
- [ ] `audit()`
- [ ] `docs_bundle(output_dir)`
- [ ] `sidecar_export(path, include_code)`
- [ ] `skill_prompt(target_path)`

**Utility tools:**
- [ ] `retrieve(ref_id, query)`
- [ ] `status()`
- [ ] `health()`
- [ ] `introspect(query, doc_path)`

### 6.3 — Security & Middleware

Port: `security/permissions.py` (105 lines), `security/rate_limiter.py` (85 lines), `middleware/audit.py` (75 lines)

- [ ] Implement `PermissionPolicy` — tool category checking (READ/MUTATE/WRITE)
- [ ] Implement `TokenBucketRateLimiter` — per-tool rate limiting
- [ ] Implement `AuditLogger` — structured invocation logging with field redaction
- [ ] Wire middleware into tool dispatch (permission check → rate limit → audit → execute)

### Phase 6 Validation

- [ ] All 35 tools callable via MCP stdio transport
- [ ] Verify: `claude mcp add contextro -- ./target/release/contextro` works
- [ ] Run full test suite (adapted from Python's 529+ tests)
- [ ] Benchmark: cold start time (target: <50ms vs Python's ~200ms)
- [ ] Benchmark: memory usage at idle (target: <50MB vs Python's ~150MB)
- [ ] Benchmark: search latency end-to-end via MCP (target: <2ms)

---

## Phase 7: Distribution + Delete Python

**Goal:** Ship the binary. Remove all Python.

### 7.1 — Build & Release

- [ ] Set up GitHub Actions CI for Rust (cargo test, cargo clippy, cargo fmt)
- [ ] Cross-compilation targets:
  - [ ] macOS arm64 (Apple Silicon)
  - [ ] macOS x86_64
  - [ ] Linux x86_64 (musl static)
  - [ ] Linux aarch64 (musl static)
  - [ ] Windows x86_64
- [ ] GitHub Releases with pre-built binaries
- [ ] `cargo install contextro` via crates.io
- [ ] Install script: `curl -fsSL https://install.contextro.dev | sh`
- [ ] Docker image (FROM scratch + static binary)

### 7.2 — Delete Python

- [ ] Remove `src/contextro_mcp/` (all Python source)
- [ ] Remove `pyproject.toml`, `setup.py`, `setup.sh`, `uv.lock`
- [ ] Remove `rust/ctx_fast/` (PyO3 extension — now internal modules)
- [ ] Remove Python test files (replaced by Rust tests)
- [ ] Update `README.md` install instructions
- [ ] Update `AGENTS.md` with new stack info
- [ ] Update skill files (no Python requirement)
- [ ] Update Docker compose files
- [ ] Update CI workflows (remove Python, add Rust)

### 7.3 — Migration Verification

- [ ] Existing MCP client configs work unchanged (`contextro` binary name preserved)
- [ ] Existing LanceDB indexes are readable (same on-disk format)
- [ ] Existing SQLite graph persistence is readable (same schema)
- [ ] `pip install contextro` still works (publishes a thin wrapper that downloads the binary)
- [ ] All 35 tools produce identical output format (JSON keys, structure)
- [ ] caveman-shrink wrapping still works

### Phase 7 Validation

- [ ] Fresh install on macOS/Linux/Windows — binary runs, MCP connects
- [ ] Upgrade from Python version — existing index loads via warm-start
- [ ] Full benchmark suite passes with improved numbers
- [ ] Binary size < 50MB (with embedded tree-sitter grammars)
- [ ] Zero runtime dependencies (single static binary)

---

## Summary

| Phase | What | Python Lines Ported | Key Crates |
|-------|------|--------------------:|------------|
| 1 | Foundation | ~965 | tree-sitter, ast-grep-core, notify |
| 2 | Indexing | ~1,385 | model2vec, lancedb, ignore, rayon, git2 |
| 3 | Search Engines | ~1,405 | tantivy, petgraph, fastembed |
| 4 | Execution + Memory + Git | ~2,165 | (internal, uses Phase 2-3 crates) |
| 5 | Analysis + Reports + Editing | ~2,285 | (internal, uses Phase 3 crates) |
| 6 | MCP Server + Tools | ~4,500 | rmcp, axum, tokio |
| 7 | Distribution | — | cross, cargo-dist |

**Total:** ~12,705 lines of Python logic to rewrite in Rust (includes server.py's 4,121 lines).
