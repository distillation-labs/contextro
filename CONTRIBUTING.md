# Contributing to Contextro

Thank you for your interest in contributing! Contextro is a fully local MCP server for code intelligence — your contributions help make AI coding agents faster, smarter, and more token-efficient.

---

## Quick Start

```bash
git clone https://github.com/jassskalkat/Contextro-MCP.git
cd Contextro-MCP

# Automated setup (creates venv, installs all deps, verifies)
./setup.sh

# Or manually:
python -m venv .venv
source .venv/bin/activate       # Windows: .venv\Scripts\activate
pip install -e ".[dev,reranker]"
pip install model2vec            # Fast embeddings (55k emb/sec)

# Verify everything works
pytest -v                        # 565+ tests
ruff check .                     # Lint
python scripts/bench_final.py    # Benchmark
```

**Requirements:** Python 3.10–3.12, pip. Optional: `ripgrep` for live grep fallback.

---

## Project Structure

```
src/contextro_mcp/
├── server.py              # All 26 MCP tools (the main entry point)
├── config.py              # CTX_* env var configuration
├── state.py               # Singleton session state + warm-start
├── indexing/
│   ├── pipeline.py        # Full + incremental indexing orchestration
│   ├── chunker.py         # Symbol → CodeChunk (with contextual enrichment)
│   ├── smart_chunker.py   # Relationship + file-context chunks
│   └── embedding_service.py  # Model2Vec / ONNX / sentence-transformers
├── engines/
│   ├── vector_engine.py   # LanceDB vector search
│   ├── bm25_engine.py     # LanceDB FTS (Tantivy)
│   ├── graph_engine.py    # rustworkx call graph
│   ├── fusion.py          # RRF fusion (vector + BM25 + graph)
│   ├── reranker.py        # FlashRank two-stage reranking
│   ├── query_cache.py     # LRU + semantic similarity cache
│   └── output_sandbox.py  # Deferred large-output storage (LRU + TTL)
├── execution/
│   ├── search.py          # Search execution engine (cache, sandbox, budget)
│   ├── response_policy.py # Progressive disclosure + search response shaping
│   ├── ast_compression.py # AST-aware snippet compression (tree-sitter)
│   └── compaction.py      # Result compaction and truncation
├── parsing/
│   ├── treesitter_parser.py   # Symbol extraction (25+ languages)
│   └── astgrep_parser.py      # Call graph extraction
├── git/
│   ├── commit_indexer.py  # Semantic commit history indexing
│   └── branch_watcher.py  # Real-time HEAD polling + auto-reindex
├── memory/
│   ├── memory_store.py    # LanceDB-backed semantic memory
│   ├── session_tracker.py # Session event tracking for snapshots
│   └── compaction_archive.py  # Searchable pre-compaction archive
└── formatting/
    ├── response_builder.py  # Token-budget-aware response formatting
    └── token_budget.py      # Token estimation and budgeting
tests/                     # 565+ pytest tests
scripts/
├── bench_final.py         # Comprehensive benchmark harness
├── benchmark_token_efficiency.py  # Token output measurement
├── benchmark_disclosure.py        # Progressive disclosure + AST compression
└── docker_healthcheck.py  # Docker health probe
```

---

## Development Workflow

### Running Tests

```bash
# Full suite (takes ~90s)
pytest -v

# Specific test file
pytest tests/test_hybrid_search.py -v

# Fast subset (skip slow embedding tests)
pytest -v -m "not slow"

# With coverage
pytest --cov=src/contextro_mcp --cov-report=term-missing
```

### Linting

```bash
ruff check .          # Check
ruff check . --fix    # Auto-fix
```

### Benchmarking

```bash
# Full benchmark (indexes the repo, runs all tools, reports token counts)
python scripts/bench_final.py

# Quick token waste analysis
python - <<'EOF'
import json, sys, os, asyncio
sys.path.insert(0, "src")
os.environ["CTX_STORAGE_DIR"] = "/tmp/ctx_bench"
# ... (see scripts/bench_final.py for full harness)
EOF
```

---

## Architecture Decisions

### Why hybrid search (vector + BM25 + graph)?

Each engine catches different things:
- **Vector**: semantic similarity ("how does auth work" → finds `verify_credentials`)
- **BM25**: exact keyword matches (function names, error messages)
- **Graph**: connectivity-based relevance (symbols called by many others are likely important)

RRF fusion with weights (0.5/0.3/0.2) combines all three. The weights are tunable via `CTX_FUSION_WEIGHT_*`.

### Why Model2Vec for embeddings?

`potion-code-16M` (Model2Vec) runs at 55,000 embeddings/sec vs 22/sec for transformer models. It achieves 99% of transformer quality on code retrieval benchmarks (NDCG@10: 0.854). The 256-dimensional vectors use 3x less memory than 768d models. The hybrid pipeline compensates for the smaller dimensions.

### Why compact serialization for callers/callees?

`"name (file:line)"` format uses ~30 tokens per entry vs ~100 for full node dicts. For a function with 15 callers, this saves ~1,050 tokens per `find_callers` call. Agents can use `find_symbol` to get full details on any specific caller.

### Research backing for search improvements

All search quality improvements are backed by published research:
- **Relevance threshold 0.40**: "The Power of Noise" SIGIR 2024 — borderline results hurt accuracy by 35%
- **Same-file diversity penalty**: SaraCoder 2025 + CrossCodeEval NeurIPS 2023
- **Bookend ordering**: Liu et al. "Lost in the Middle" TACL 2023 — 30%+ accuracy improvement
- **Pre-rerank pool 50**: NVIDIA "Enhancing RAG with Re-Ranking" — 14% accuracy improvement
- **Contextual chunk enrichment**: Anthropic "Contextual Retrieval" Sep 2024 — 35-49% fewer retrieval failures
- **Sufficiency signal**: Google "Sufficient Context" ICLR 2025 — 10% hallucination reduction

---

## Adding a New Tool

1. Add the tool function inside `create_server()` in `server.py` using `@mcp.tool()`:

```python
@mcp.tool()
def my_tool(
    param: Annotated[str, "Description of param"],
) -> dict[str, Any]:
    """Tool description shown to agents."""
    guard_err = _guard("my_tool")
    if guard_err:
        return guard_err
    
    # Your implementation
    return {"result": "..."}
```

2. Add permission category in `security/permissions.py` if needed.

3. Write tests in `tests/test_my_tool.py` following the existing pattern:

```python
import asyncio
from tests.conftest import _call_tool, _setup_indexed

class TestMyTool:
    def test_basic(self, mini_codebase, tmp_path):
        async def run():
            mcp, _, _ = await _setup_indexed(mini_codebase, tmp_path / ".contextro")
            return await _call_tool(mcp, "my_tool", {"param": "value"})
        result = asyncio.run(run())
        assert "error" not in result
```

4. Add to `introspect` tool's `tools_doc` dict.

5. Update `SKILL.md` tool decision table.

---

## Adding a New Language

1. Add the language to `parsing/language_registry.py`:

```python
"mylang": LanguageConfig(
    name="mylang",
    extensions=[".ml"],
    treesitter_name="mylang",
    astgrep_id="mylang",
    function_patterns=["function_definition"],
    class_patterns=["class_definition"],
    ...
)
```

2. Verify tree-sitter-languages supports it: `python -c "from tree_sitter_languages import get_parser; get_parser('mylang')"`

3. Add test cases in `tests/test_treesitter_parser.py`.

---

## Performance Guidelines

- **Never load models eagerly** — all heavy deps use lazy imports
- **Unload models after indexing** — `embedding_service.unload()` frees RAM
- **Batch embedding** — always use `embed_batch()`, never loop `embed()`
- **Cap all list outputs** — use `[:20]` or similar to prevent token bloat
- **Use compact serialization** — `_serialize_node_compact()` for list contexts
- **Test token output size** — add a size assertion in tests for new tools

---

## Submitting a PR

1. Fork the repo and create a branch: `git checkout -b feat/my-improvement`
2. Make your changes with tests
3. Run `pytest -v` and `ruff check .` — both must pass
4. Run `python scripts/bench_final.py` and include output in your PR description
5. Open a PR with:
   - What the change does
   - Why it's needed (link research if applicable)
   - Benchmark results (before/after token counts)
   - Test coverage

### PR Checklist

- [ ] Tests pass (`pytest -v`)
- [ ] Lint passes (`ruff check .`)
- [ ] No new dependencies without justification
- [ ] Token output size not increased without justification
- [ ] Benchmark results included
- [ ] CHANGELOG.md updated

---

## Reporting Issues

Please include:
- Python version (`python --version`)
- Contextro version (`pip show contextro`)
- Operating system
- Minimal reproduction case
- Full error traceback

---

## Code Style

- **ruff** for linting (line-length=100)
- **Frozen dataclasses** for immutable models
- **ABC interfaces** for swappable components
- **Thread-safe singletons** with locks
- **Lazy imports** for heavy dependencies (torch, lancedb, etc.)
- **Type annotations** on all public functions

---

## License

MIT — contributions are accepted under the same license.
