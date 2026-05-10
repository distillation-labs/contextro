# Contextro: Full Rust Rewrite Architecture

**Goal:** Remove Python entirely. Single compiled Rust binary. No interpreter, no GIL, no ONNX Runtime, no Python bindings overhead.

---

## Why This Is the Right Call

The current architecture is Python orchestrating Rust (ctx_fast) for the fast parts. The problem: every boundary crossing between Python and Rust costs serialization, GIL acquisition, and interpreter overhead. The hot path — `search()` — crosses this boundary multiple times per call.

A full Rust binary eliminates:
- Python interpreter startup (~200ms cold start)
- GIL contention on parallel operations
- PyO3 serialization overhead on every ctx_fast call
- ONNX Runtime Python bindings overhead on every embedding
- Python asyncio overhead on every MCP I/O operation
- 2–4x excess memory from Python's GC and object model

Expected gains over current Python+ctx_fast hybrid:
- **Indexing**: 3–5x faster (embedding inference is the bottleneck; native Rust removes it)
- **Search latency**: 5–10x faster (no Python overhead on the hot path)
- **Memory**: 2–4x lower (no Python runtime, no GC headroom)
- **Cold start**: ~10x faster (no interpreter, no model loading via Python)
- **Binary size**: single static binary, ships as one file

---

## Crate Selection

Every crate chosen is production-proven. No experimental dependencies.

### MCP Server Layer

**`rmcp`** (official Rust SDK for MCP, 4.7M downloads)
- `modelcontextprotocol/rust-sdk` — the official SDK
- Macro-driven tool registration: `#[tool]` attribute on async functions
- Supports stdio and HTTP transports natively
- Tokio-based async runtime underneath

```toml
rmcp = { version = "0.1", features = ["server", "transport-io", "macros"] }
tokio = { version = "1", features = ["full"] }
```

### Embedding Inference

**`model2vec`** (official Rust implementation by MinishLab)
- `MinishLab/model2vec-rs` — the official Rust crate for Model2Vec
- Loads potion-code-16m directly from HuggingFace or local path
- Static embedding lookup + mean pooling — no transformer forward pass
- Benchmarked at **50,000 requests/second** throughput in Rust
- Zero Python, zero ONNX Runtime

For transformer models (jina-code, bge-small-en) when needed:
**`fastembed`** (Qdrant's Rust embedding library)
- ONNX models via `ort` (Rust ONNX Runtime bindings)
- Same models as Python fastembed, native Rust inference
- 2–4x faster than Python ONNX path

```toml
model2vec = "0.1"          # for potion-code-16m (default)
fastembed = "4"            # for transformer models (optional)
```

### Vector Search

**`lancedb`** (native Rust crate — LanceDB is written in Rust)
- The Python LanceDB you use today is a wrapper around this
- Direct Rust API eliminates the Python binding overhead entirely
- Same on-disk format — existing indexes are compatible
- HNSW + IVF-PQ, SIMD-accelerated distance computation

```toml
lancedb = "0.14"
arrow = { version = "53", features = ["ipc"] }
```

### Full-Text / BM25 Search

**`tantivy`** (Lucene-equivalent in Rust, used by Quickwit and ParadeDB)
- Production-proven: powers Quickwit (distributed search), ParadeDB (Postgres FTS)
- BM25 scoring, inverted index, incremental indexing
- Replaces the current Python BM25 engine entirely
- Significantly faster than any Python BM25 implementation

```toml
tantivy = "0.22"
```

### Code Parsing

**`tree-sitter`** (native Rust bindings — tree-sitter core is C, Rust bindings are first-class)
- `tree-sitter` crate + language grammars (`tree-sitter-python`, `tree-sitter-javascript`, etc.)
- Same parsing quality as current Python tree-sitter, zero Python overhead
- Incremental parsing built-in (re-parse only changed regions)

**`ast-grep`** (the `sg` library crate)
- ast-grep is written in Rust — use it as a library, not a subprocess
- Replaces the current Python `ast-grep` subprocess calls entirely

```toml
tree-sitter = "0.23"
tree-sitter-python = "0.23"
tree-sitter-javascript = "0.23"
tree-sitter-typescript = "0.23"
tree-sitter-rust = "0.23"
tree-sitter-go = "0.23"
# ... all supported languages
ast-grep-core = "0.27"
```

### Call Graph / Symbol Graph

**`petgraph`** (the standard Rust graph library, used in compilers and code intelligence tools)
- Directed graph with O(1) node/edge lookup
- Built-in algorithms: DFS, BFS, SCC (for circular dependency detection), topological sort
- Replaces `rustworkx` (which is itself a Rust library wrapped in Python)
- Persistence via `rusqlite` (same SQLite backend, same schema)

```toml
petgraph = "0.6"
```

### Persistence

**`rusqlite`** for graph persistence, memory store, and metadata
- Ergonomic SQLite bindings, battle-tested
- Bundled SQLite (no system dependency)
- Used by Qdrant for their custom key-value store internals

**`lancedb`** for vector + FTS index (same as above)

```toml
rusqlite = { version = "0.31", features = ["bundled"] }
```

### File Operations (already in Rust — keep as internal modules)

The current `ctx_fast` crate becomes internal modules in the main binary:
- `ignore` crate for .gitignore-aware parallel file walking (same as ripgrep)
- `rayon` for parallel file hashing and mtime scanning
- `xxhash-rust` for xxHash3 content hashing
- `git2` for git operations (replaces subprocess git calls)

```toml
ignore = "0.4"
rayon = "1"
xxhash-rust = { version = "0.8", features = ["xxh3"] }
git2 = { version = "0.19", default-features = false }
```

### Async Runtime & HTTP

**`tokio`** — the standard async runtime for Rust production systems
**`axum`** — for HTTP transport mode (Docker/team deployments)

```toml
tokio = { version = "1", features = ["full"] }
axum = "0.7"
tower = "0.4"
```

### Serialization

**`serde`** + **`serde_json`** — the standard. Zero overhead, compile-time checked.

```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### Configuration

**`config`** crate for `CTX_`-prefixed environment variable loading (replaces Python dataclass + os.environ).

```toml
config = "0.14"
```

---

## Workspace Structure

```
contextro/
├── Cargo.toml                    # workspace root
├── crates/
│   ├── contextro-core/           # domain types, traits, errors
│   │   └── src/
│   │       ├── models.rs         # CodeChunk, Symbol, SearchResult, etc.
│   │       ├── graph_models.rs   # GraphNode, GraphEdge
│   │       └── errors.rs
│   │
│   ├── contextro-config/         # Settings, CTX_ env vars
│   │   └── src/lib.rs
│   │
│   ├── contextro-parsing/        # tree-sitter + ast-grep
│   │   └── src/
│   │       ├── treesitter.rs     # symbol extraction, chunk boundaries
│   │       ├── astgrep.rs        # pattern search + rewrite
│   │       └── language.rs       # language registry
│   │
│   ├── contextro-indexing/       # pipeline, chunker, embedding, file ops
│   │   └── src/
│   │       ├── pipeline.rs       # IndexingPipeline (incremental + full)
│   │       ├── chunker.rs        # smart chunker
│   │       ├── embedding.rs      # model2vec + fastembed trait
│   │       ├── file_scanner.rs   # ignore + rayon (from ctx_fast)
│   │       └── git.rs            # git2 ops (from ctx_fast)
│   │
│   ├── contextro-engines/        # search engines
│   │   └── src/
│   │       ├── vector.rs         # lancedb vector search
│   │       ├── bm25.rs           # tantivy BM25
│   │       ├── graph.rs          # petgraph call graph
│   │       ├── fusion.rs         # RRF + entropy-adaptive weighting
│   │       ├── reranker.rs       # cross-encoder reranking via fastembed
│   │       └── cache.rs          # LRU query cache
│   │
│   ├── contextro-memory/         # remember/recall/compact/archive
│   │   └── src/
│   │       ├── store.rs          # rusqlite memory store
│   │       └── archive.rs        # compaction archive
│   │
│   ├── contextro-git/            # commit indexer, branch watcher, cross-repo
│   │   └── src/
│   │       ├── commit_indexer.rs
│   │       ├── branch_watcher.rs
│   │       └── cross_repo.rs
│   │
│   ├── contextro-tools/          # all 35 MCP tool implementations
│   │   └── src/
│   │       ├── search.rs
│   │       ├── index.rs
│   │       ├── find_symbol.rs
│   │       ├── find_callers.rs
│   │       ├── find_callees.rs
│   │       ├── explain.rs
│   │       ├── impact.rs
│   │       ├── code.rs           # AST operations
│   │       ├── overview.rs
│   │       ├── architecture.rs
│   │       ├── analyze.rs
│   │       ├── memory.rs         # remember/recall/forget
│   │       ├── knowledge.rs
│   │       ├── git_tools.rs      # commit_search/commit_history
│   │       ├── session.rs        # compact/session_snapshot/restore
│   │       ├── sandbox.rs        # retrieve
│   │       ├── repo.rs           # repo_add/remove/status
│   │       ├── reports.rs        # audit/docs_bundle/sidecar_export
│   │       └── status.rs         # status/health
│   │
│   └── contextro-server/         # MCP server binary (rmcp + axum)
│       └── src/
│           ├── main.rs           # entry point, transport selection
│           ├── stdio.rs          # stdio transport (local use)
│           └── http.rs           # HTTP transport (Docker/team)
│
└── tests/                        # integration tests
    ├── indexing_tests.rs
    ├── search_tests.rs
    └── e2e_tests.rs
```

---

## Hot Path: search() in Pure Rust

Current Python path (every call):
```
Python asyncio → Python search() → PyO3 boundary → ctx_fast (Rust) → back to Python
→ Python LanceDB bindings → Python BM25 → Python rustworkx → Python RRF fusion
→ Python reranker (ONNX via Python) → Python response builder → MCP response
```

New Rust path:
```
Tokio task → search handler → parallel: [lancedb query | tantivy query | petgraph traversal]
→ RRF fusion → optional reranker → response builder → MCP response
```

No boundaries. No serialization. Everything in the same process, same memory space.

---

## Embedding: model2vec-rs

potion-code-16m is a Model2Vec model. The official Rust implementation (`model2vec-rs`) loads it natively:

```rust
use model2vec::StaticModel;

let model = StaticModel::from_pretrained("minishlab/potion-code-16m", None)?;
let embeddings = model.encode(&["fn authenticate(token: &str) -> bool"], true);
```

No ONNX. No Python. No tokenizer subprocess. Static embedding lookup + mean pooling in pure Rust. Benchmarked at 50,000 req/s — faster than the current Python ONNX path by a significant margin.

---

## MCP Tool Registration with rmcp

```rust
use rmcp::{ServerHandler, tool, model::*};

#[derive(Clone)]
struct ContextroServer {
    state: Arc<AppState>,
}

#[rmcp::tool_router]
impl ContextroServer {
    #[tool(description = "Semantic + keyword + graph hybrid search")]
    async fn search(&self, query: String, mode: Option<String>) -> Result<CallToolResult> {
        // pure Rust: lancedb + tantivy + petgraph + RRF
    }

    #[tool(description = "Index a codebase")]
    async fn index(&self, path: String) -> Result<CallToolResult> {
        // pure Rust: ignore + rayon + model2vec + lancedb + tantivy
    }
    // ... all 35 tools
}
```

---

## Migration Phases

### Phase 1 — Core infrastructure (no tools yet)
Build the crates: `contextro-core`, `contextro-config`, `contextro-parsing`, `contextro-indexing`.
Milestone: can index a codebase in Rust, producing the same chunks as Python.
Validation: compare chunk output against Python implementation on the same repo.

### Phase 2 — Search engines
Build `contextro-engines`: vector (lancedb), BM25 (tantivy), graph (petgraph), fusion, cache.
Milestone: `search()` works end-to-end in Rust with parity results.
Validation: run the existing 20-query benchmark suite, verify MRR ≥ 1.000.

### Phase 3 — All 35 tools
Build `contextro-tools`. Port each tool from Python, one by one.
Milestone: full feature parity with Python implementation.
Validation: run existing test suite (529+ tests) adapted to Rust.

### Phase 4 — Server binary
Build `contextro-server` with rmcp. Wire stdio and HTTP transports.
Milestone: drop-in replacement for `contextro` CLI command.
Validation: existing MCP client configs work unchanged.

### Phase 5 — Delete Python
Remove `src/contextro_mcp/`, `pyproject.toml`, `setup.py`, `uv.lock`.
Update `README.md` install instructions (`cargo install contextro` or pre-built binary).

---

## What Gets Deleted

| Python module | Rust replacement |
|---|---|
| `indexing/embedding_service.py` | `contextro-indexing/embedding.rs` (model2vec-rs) |
| `indexing/pipeline.py` | `contextro-indexing/pipeline.rs` |
| `indexing/chunker.py` + `smart_chunker.py` | `contextro-indexing/chunker.rs` |
| `engines/vector_engine.py` | `contextro-engines/vector.rs` (lancedb native) |
| `engines/bm25_engine.py` | `contextro-engines/bm25.rs` (tantivy) |
| `engines/graph_engine.py` | `contextro-engines/graph.rs` (petgraph) |
| `engines/fusion.py` | `contextro-engines/fusion.rs` |
| `engines/reranker.py` | `contextro-engines/reranker.rs` (fastembed) |
| `engines/query_cache.py` | `contextro-engines/cache.rs` |
| `parsing/treesitter_parser.py` | `contextro-parsing/treesitter.rs` |
| `parsing/astgrep_parser.py` | `contextro-parsing/astgrep.rs` |
| `git/commit_indexer.py` | `contextro-git/commit_indexer.rs` (git2) |
| `git/branch_watcher.py` | `contextro-git/branch_watcher.rs` |
| `memory/memory_store.py` | `contextro-memory/store.rs` (rusqlite) |
| `execution/search.py` | `contextro-tools/search.rs` |
| `server.py` (35 tools, 165k lines) | `contextro-tools/` + `contextro-server/` |
| `rust/ctx_fast/` (PyO3 extension) | internal modules in `contextro-indexing` |
| All Python deps (FastMCP, LanceDB Python, etc.) | gone |

---

## Distribution

Current: `pip install contextro` (requires Python 3.10–3.12, Rust toolchain for source installs)

New: pre-built binaries via GitHub Releases for macOS (arm64, x86_64), Linux (x86_64, aarch64), Windows (x86_64). Single static binary, no runtime dependencies.

```bash
# Install via cargo
cargo install contextro

# Or download pre-built binary
curl -fsSL https://install.contextro.dev | sh
```

The binary is the MCP server. Users point their MCP config at it exactly as today:
```bash
claude mcp add contextro -- contextro
```

Nothing changes for users. Everything changes underneath.

---

## Key Risks and Mitigations

**tree-sitter language coverage**: Python's `tree-sitter-languages` bundles 100+ grammars. Rust requires adding each grammar crate individually. Mitigation: start with the top 15 languages (covers >95% of real codebases), add more incrementally.

**LanceDB Rust API maturity**: The Rust API is the canonical implementation (Python is a wrapper), but the Rust API surface is smaller than Python's. Mitigation: LanceDB's core team actively maintains the Rust crate; any gaps are bugs, not design decisions.

**model2vec-rs model coverage**: model2vec-rs supports potion-code-16m (the default) and any Model2Vec model. For transformer models (jina-code, bge-small-en), use fastembed-rs. Mitigation: the default model is fully covered; transformer models are optional extras.

**Test suite migration**: 529+ Python tests need Rust equivalents. Mitigation: port tests in parallel with implementation, phase by phase. The existing test suite serves as the specification.
