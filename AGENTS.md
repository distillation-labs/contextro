# Contextro Agent Guidelines

## Architecture
Single compiled Rust binary. 37 MCP tools, <50MB RAM at idle.

## Stack
- **rmcp**: Official Rust MCP SDK (stdio + HTTP transports)
- **model2vec**: Static embeddings (potion-base-8M, 256d, 50k emb/sec)
- **lancedb**: Vector search (native Rust, HNSW + IVF-PQ)
- **tantivy**: Full-text BM25 search (inverted index)
- **petgraph**: In-memory directed call graph (O(1) lookups, SCC, BFS)
- **tree-sitter**: Code parsing (15+ languages)
- **git2**: Git operations via libgit2 (no subprocess)
- **rusqlite**: Memory persistence (bundled SQLite)
- **tokio + axum**: Async runtime + HTTP transport

## Workspace Structure
```
crates/
├── contextro-core/       # Domain types, traits, errors
├── contextro-config/     # CTX_ env var settings
├── contextro-parsing/    # tree-sitter symbol extraction
├── contextro-indexing/   # Pipeline, chunker, embeddings, file scanner
├── contextro-engines/    # BM25, graph, fusion, cache, sandbox
├── contextro-memory/     # SQLite memory store, compaction archive
├── contextro-git/        # Commit indexer, branch watcher
├── contextro-tools/      # All 37 tool implementations
└── contextro-server/     # MCP server binary (rmcp + axum)
```

## Key Constraints
- **Rust 2021 edition**, workspace with 9 crates
- Single binary output (~9MB stripped)
- No Python, no interpreter, no GIL
- Models loaded lazily on first use
- All search engines thread-safe (RwLock/Mutex)

## Performance
- **Cold start**: <50ms
- **Search latency**: <1ms (warm BM25 + graph)
- **Indexing**: ~2s for 3,000 files (tree-sitter parsing + BM25 indexing)
- **Memory at idle**: <50MB

## Code Style
- `cargo fmt` + `cargo clippy`
- Workspace dependencies in root Cargo.toml
- Error handling via `thiserror` + `anyhow`
- Structured logging via `tracing`
- Thread-safe state via `parking_lot` + `Arc`

## Testing
- `cargo test` from crates/ directory
- Unit tests in each crate (mod tests)
- Integration test in contextro-indexing/tests/
- `cargo build --release` for production binary
