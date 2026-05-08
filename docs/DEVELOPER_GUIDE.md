# Developer Guide

## Development Setup

```bash
# Clone the repository
git clone https://github.com/jassskalkat/Contextro.git
cd Contextro

# Create virtual environment
python3.12 -m venv .venv
source .venv/bin/activate

# Install with dev dependencies
pip install -e ".[dev,reranker,model2vec]"

# Verify installation
pytest -v
ruff check .
contextro --help
```

## Project Structure

```
Contextro/
├── src/contextro_mcp/      # Source code
│   ├── server.py           # FastMCP server, tool registration, transport entry point
│   ├── config.py           # Settings with CTX_ env prefix
│   ├── state.py            # Session state singleton
│   ├── core/               # Data models, interfaces, exceptions
│   ├── execution/          # Shared execution/runtime layer for search flows
│   ├── parsing/            # tree-sitter + ast-grep parsers
│   ├── engines/            # Vector, BM25, graph, fusion, reranker
│   ├── analysis/           # Code complexity and quality analysis
│   ├── memory/             # Semantic memory store
│   ├── indexing/           # Pipeline, embedding service, chunker
│   ├── formatting/         # Token budget, response builder
│   └── persistence/        # SQLite graph persistence
├── tests/                  # Unit, integration, and regression tests
├── docs/                   # Architecture, ADRs, research notes
│   ├── adr/                # 11 Architecture Decision Records
│   └── research/           # Research notes on libraries
├── pyproject.toml          # Build config, dependencies
└── LICENSE                 # MIT License
```

## Running Tests

```bash
# Full suite
pytest -v

# Skip slow performance benchmarks
pytest -v -m "not slow"

# Run specific test file
pytest tests/test_security.py -v

# Run with coverage (if pytest-cov installed)
pytest --cov=contextro_mcp --cov-report=term-missing
```

### Test Categories

| File | Tests | Purpose |
|------|-------|---------|
| `test_tools_basic.py` | 11 | Core MCP tools (index, search, status) |
| `test_graph_tools.py` | 14 | Graph tools (find_symbol, find_callers, find_callees) |
| `test_analyze_tool.py` | 5 | Code analysis tool |
| `test_impact_tool.py` | 6 | Impact analysis tool |
| `test_explain_tool.py` | 8 | Explain tool with verbosity levels |
| `test_hybrid_search.py` | 15 | Hybrid search, fusion, reranking |
| `test_memory_tools.py` | 12 | Remember, recall, forget tools |
| `test_security.py` | 17 | Input validation, SQL injection, path traversal |
| `test_e2e.py` | 10 | End-to-end lifecycle, corrupt index recovery |
| `test_performance.py` | 4 | Performance benchmarks (marked slow) |
| `test_memory_usage.py` | 5 | RSS monitoring and memory stability |
| `test_vector_engine.py` | 14 | LanceDB vector engine CRUD |
| `test_pipeline.py` | 19 | Indexing pipeline, incremental reindex |
| `test_chunker.py` | 22 | Symbol-to-chunk conversion |
| ... | ... | ... |

### Test Conventions

- **Naming:** `test_{function}_{scenario}`
- **Fixtures:** Shared setup in `tests/conftest.py` (`mini_codebase`, `_setup_indexed`, `_call_tool`)
- **No mocking of core models** (Symbol, ParsedFile, etc.)
- **Both happy path and error cases** required
- **Fast:** Each test <5s, full suite <30s
- **File system tests** use `tmp_path` fixture

## Linting

```bash
# Check for issues
ruff check .

# Auto-fix
ruff check --fix .

# Configuration in pyproject.toml: E, F, W, I rules, 100 char line length
```

Repo-wide `ruff check .` is expected to pass. The only file-level allowances are explicit `E501` exceptions in `pyproject.toml` for legacy tool-doc-heavy modules where wrapping the embedded help text would hurt readability more than it would help.

## Live Dev Loop

Use `docker-compose.dev.yml` for MCP development. It runs `scripts/dev_http_server.py`, which watches `src/` and `scripts/` and restarts the HTTP server automatically when code changes land.

```bash
export CTX_CODEBASE_HOST_PATH=/absolute/path/to/repo-under-test
docker compose -f docker-compose.dev.yml up --build
```

Notes:

- source edits do not require an image rebuild
- dependency or base-image changes still require `--build`
- `CTX_AUTO_WARM_START=true` keeps the indexed state across restarts
- schema changes can still require the MCP client to reconnect

## Benchmarks

```bash
# Synthetic workflow benchmark for indexing/search token output
python scripts/benchmark_token_efficiency.py

# Retrieval-quality scorecard (Recall@K / MRR by search mode)
python scripts/benchmark_retrieval_quality.py --path src --query-limit 20

# Compare chunking profiles (minimal vs contextual vs smart)
python scripts/benchmark_chunk_profiles.py --path src --query-limit 20

# Real-world benchmark against browser-use
BROWSER_USE_PATH=/path/to/browser-use python scripts/benchmark_browser_use.py
```

The benchmark harness now lives in `scripts/benchmark_utils.py` so the benchmark scripts share the same patched session setup. Run them from the activated project venv with `python`, or use an explicit Python `3.10`-`3.12` interpreter such as `python3.11` or `python3.12`.

Current recommendation: keep the default `smart` chunk profile unless you have a concrete index-size constraint. In the current `src` / `20`-query benchmark, `smart` matched the best hybrid quality (`MRR 0.625`, `Recall@5 0.9`) while using fewer average response tokens than `contextual` (`491` vs `510`). `minimal` reduced token cost further but dropped hybrid `Recall@5` to `0.55`.

For search/snippet-compression changes, run the token benchmark and retrieval scorecard with the **same query limit before and after the change**. Keep the optimization only if token output drops without a meaningful Recall@K / MRR regression.

Search response shaping is split between `execution/compaction.py` (snippet compression) and
`execution/response_policy.py` (inline budgeting, preview shaping, sandbox handoff). Keep new
token-efficiency behavior in these modules instead of growing `server.py` or `execution/search.py`.

## Alpha Deploys

Pushes to the `alpha` branch run `.github/workflows/alpha.yml`:

1. `ruff check .`
2. `pytest -v -m "not slow"`
3. build and push `ghcr.io/<owner>/contextro-mcp:alpha`
4. build and push `ghcr.io/<owner>/contextro-mcp:alpha-<short-sha>`
5. optionally deploy the new alpha image to a remote host over SSH

Remote alpha deploy assets live in `deploy/alpha/`.

Remote deploy secrets:

- required: `ALPHA_SSH_HOST`, `ALPHA_SSH_USER`, `ALPHA_SSH_PRIVATE_KEY`
- optional: `ALPHA_SSH_PORT`, `ALPHA_REMOTE_DIR`, `ALPHA_HTTP_PORT`

Stable releases stay in `.github/workflows/publish.yml` and only publish PyPI / `latest` when the GitHub Release is not marked as a prerelease.

## Adding a New MCP Tool

1. Add the tool function inside `create_server()` in `server.py`:
   ```python
   @mcp.tool()
   def my_tool(param: str) -> dict[str, Any]:
       """Tool description for MCP clients."""
       # Validate input
       err = _validate_query(param)
       if err:
           return err

       # Require indexing if needed
       state, err = _require_indexed()
       if err:
           return err

       # Tool logic here
       return {"result": "..."}
   ```

2. Add input validation if the tool accepts user input (paths, names, queries).

3. If the tool shares orchestration with an existing tool (search, caching, token budgeting, sandboxing), extract that logic into `execution/` instead of growing `server.py`.

4. Write tests in a new `tests/test_my_tool.py` or add to an existing file.

5. Update the tools table in `README.md`.

6. Register the tool in `TOOL_PERMISSIONS` in `security/permissions.py`.

7. Update any architecture/docs references that describe the tool surface.

## Adding a New Engine

1. Create `engines/my_engine.py` implementing the `IEngine` interface (or a custom interface).

2. Wire it into `IndexingPipeline.__init__()` in `pipeline.py`.

3. Expose it via `SessionState` in `state.py` (add property + setter).

4. Wire it into `server.py` in the `index` tool (store reference on state).

5. Write tests in `tests/test_my_engine.py`.

## Architecture Decision Records

All significant design decisions are documented in `docs/adr/`. When making a key decision:

1. Copy `docs/adr/ADR-000-template.md`
2. Number it sequentially (ADR-012, etc.)
3. Document: Context, Decision, Alternatives Considered, Consequences
4. Add a reference to `CLAUDE.md` under "Key Decisions"

## Key Design Patterns

### Singleton State
`state.py` uses a module-level singleton. Always access via `get_state()`. Reset with `reset_state()` in tests.

### Lazy Loading
Engines are `None` until indexing runs. The embedding model is loaded during indexing and unloaded afterward to free RAM. FlashRank loads on first rerank call.

### Defensive Validation
All tool inputs are validated at entry (null bytes, length limits, path traversal). SQL filter values are escaped. This happens in `server.py` before any business logic.

### Graceful Degradation
If FlashRank isn't installed, reranking falls through to passthrough. If BM25 fails, hybrid search continues with remaining engines. Each engine failure is logged but doesn't crash the server.

## Debugging

### Enable Debug Logging
```bash
CTX_LOG_LEVEL=DEBUG contextro
```

### JSON Logging (for structured log analysis)
```bash
CTX_LOG_FORMAT=json contextro
```

### Check Index Health
Use the `status` tool to verify:
- `indexed: true` — Codebase has been indexed
- `vector_chunks > 0` — Vector store has data
- `bm25_fts_ready: true` — Full-text search index built
- `graph.total_nodes > 0` — Graph has structure
- `memory.peak_rss_mb < 350` — Within memory budget

## Code Provenance


- **Contextro** — Original author: . Licensed under MIT.
- **Contextro** — Original author: [](https://github.com/).

| Component | Origin | Files |
|-----------|--------|-------|
| Core models (Symbol, ParsedFile, Memory) | Contextro | `core/models.py`, `core/interfaces.py`, `core/exceptions.py` |
| tree-sitter parser | Contextro | `parsing/treesitter_parser.py` |
| Embedding service (ONNX) | Contextro | `indexing/embedding_service.py` |
| Parallel indexer | Contextro | `indexing/parallel_indexer.py` |
| Graph models (UniversalNode) | Contextro | `core/graph_models.py` |
| ast-grep parser | Contextro | `parsing/astgrep_parser.py` |
| Graph engine (rustworkx) | Contextro | `engines/graph_engine.py` |
| Code analyzer | Contextro | `analysis/code_analyzer.py` |
| File watcher | Contextro | `parsing/file_watcher.py` |
| Language registry | Both (merged) | `parsing/language_registry.py` |

Everything else (vector engine, BM25, fusion, reranker, memory store, pipeline, formatting, persistence, server, all hardening) was written fresh for Contextro.

## Release Process

Stable release:

1. Update version in `pyproject.toml`
2. Run full test suite: `pytest -v`
3. Run linter: `ruff check .`
4. Run Snyk scan
5. Build: `python -m build`
6. Test install: `pip install dist/contextro_mcp-*.whl` in a clean venv
7. Create a non-prerelease GitHub Release to publish PyPI + `ghcr.io/...:latest`

Alpha release:

1. Merge or push the candidate to `alpha`
2. Let `.github/workflows/alpha.yml` publish `:alpha` and `:alpha-<short-sha>`
3. If remote alpha secrets are configured, let the workflow update the hosted alpha endpoint
