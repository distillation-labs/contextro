# Contextro Agent Guidelines

## Architecture
Single unified MCP server. 26 tools, <350MB RAM.

## Stack
- **LanceDB**: vectors + FTS (disk-backed, mmap)
- **Model2Vec**: default embedding (potion-code-16M, 256d, 55k emb/sec)
- **ONNX Runtime**: optional for transformer models (bge-small-en, jina-code)
- **rustworkx**: in-memory directed graph (Rust-backed, O(1) lookups)
- **tree-sitter + ast-grep**: dual parsing (symbols + relationships)
- **ctx_fast**: Rust extension for parallel file ops, xxHash3, git ops
- **SQLite**: graph persistence for warm-start recovery

## Embedding Models
| Model | Dims | Speed | Quality | Use case |
|-------|------|-------|---------|----------|
| `potion-code-16m` (default) | 256 | 55k/sec | 99% of SOTA | Code search, fast + accurate |
| `potion-8m` | 256 | 80k/sec | Good | Maximum speed, general text |
| `bge-small-en` | 384 | 22/sec | Better | Legacy, smaller codebases |
| `jina-code` | 768 | 15/sec | Best | Maximum quality, code-specific |

## Key Constraints
- **Python 3.10–3.12** (tree-sitter-languages constraint)
- **pip** (comes with Python); model2vec for fast embeddings
- All modules lazy-import heavy deps
- Models unloaded after indexing (`del model; gc.collect()`)
- Batch embedding size=128, file batch=200
- Graph payloads: {id, name, type, file, line} only
- Graph persisted to SQLite for warm-start on restart

## Performance
- **Indexing**: 3,349 files in 8.1s (potion-code-16m) or 6.8s (potion-8m)
- **Incremental reindex**: 22ms when no files changed
- **Search**: <2ms query latency (warm index)
- **File discovery**: 15ms for 3,349 files (Rust ctx_fast)
- **Token efficiency**: 65% reduction vs raw file reading

## Token Efficiency Features
- Progressive snippet truncation (top results get more budget)
- Result deduplication (overlapping line ranges from same file)
- Relevance threshold filtering (drop < 30% of top score)
- Adaptive result count (fewer results when confidence is high)
- Query result cache (LRU + TTL, 23%+ hit rate)
- Compact serialization (short keys, omit defaults/empty)
- Session snapshot for context recovery after compaction
- Output sandbox for deferred content retrieval
- Universal progressive disclosure (~44% token savings on large responses)
- AST-aware snippet compression (~73% reduction on code previews)
- Searchable compaction archive for context recovery
- Optional TOON encoding (CTX_OUTPUT_FORMAT=toon, +10% savings)

## Testing
- Tests first (spec-driven)
- `pytest -v` for all tests
- `ruff check .` must pass
- 529+ tests passing

## Code Style
- ruff for linting (line-length=100)
- Frozen dataclasses for immutable models
- ABC interfaces for swappable components
- Thread-safe singletons with locks
