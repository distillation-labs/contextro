# Contextro

**Give your AI coding agent a brain.**

Contextro is a local MCP server that connects your AI agent (Claude, Cursor, Windsurf, etc.) to your codebase. Instead of reading files and guessing, your agent can search by meaning, trace call graphs, check what breaks before a refactor, search git history, and remember context across sessions — all running locally on your machine.

No cloud. No API keys. No data leaves your machine.

---

## Why Contextro?

Without Contextro, your agent reads 5–10 full files to find one function. With Contextro, it finds the exact chunk in one search call.

```
Without:  grep "auth" → read auth.py → read middleware.py → read utils.py → ...
With:     search("authentication flow") → exact result in <2ms
```

| Task | Without Contextro | With Contextro | Savings |
|---|---|---|---|
| Find a function | Read 5 files (~5000 tokens) | `search()` (~116 tokens) | **43x** |
| Trace callers | grep + read 3 files (~3000 tokens) | `find_callers()` (~6 tokens) | **500x** |
| Understand a class | Read file + grep (~2000 tokens) | `explain()` (~43 tokens) | **47x** |
| Check what breaks | Manual audit (~8000 tokens) | `impact()` (~300 tokens) | **27x** |

---

## Install

```bash
pip install contextro
```

**Requirements:** Python 3.10–3.12

PyPI wheels bundle the `ctx_fast` Rust extension on supported platforms, so the default install gets the native file discovery, hashing, mtime, and git fast paths automatically.

Optional extras for better performance:
```bash
pip install contextro[reranker]   # Better search quality (FlashRank reranking)
```

The default install already includes the `potion-code-16m` Model2Vec embedding path used by Contextro out of the box.

For source installs (`pip install -e .`, `./setup.sh`), install a Rust toolchain first so the bundled `ctx_fast` extension can compile.

---

## Connect to Your Agent

### Claude Code
```bash
claude mcp add contextro -- contextro
```

### Claude Desktop
Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "contextro": {
      "command": "contextro"
    }
  }
}
```

### Cursor / Windsurf / Any MCP Client
Add to your MCP configuration:
```json
{
  "contextro": {
    "command": "contextro",
    "transport": "stdio"
  }
}
```

---

## Getting Started

```
1. Tell your agent: "Index this project at /path/to/your/project"
2. Wait a few seconds (agent will poll status automatically)
3. Ask anything about your code
```

That's it. The index persists on disk — you only need to do this once per project.

---

## What You Can Do

### Search your codebase by meaning

```
search("how does authentication work")
search("database connection pool", language="python")
search("TokenBudget", mode="bm25")   ← exact keyword match
```

Contextro runs semantic search, keyword search, and graph search in parallel, then fuses the results. You get the code snippet, file path, line number, and confidence level.

If results are large, you'll get a compact preview plus a `sandbox_ref` — call `retrieve(sandbox_ref)` to get the full set.

---

### Find any symbol

```
find_symbol("IndexingPipeline")           ← exact match
find_symbol("auth", exact=False)          ← fuzzy search
```

Returns the definition location, caller count, and top callers. When multiple definitions share the same name, results are grouped per-definition with file context.

---

### Trace the call graph

```
find_callers("authenticate")    ← who calls this function?
find_callees("authenticate")    ← what does this function call?
```

Returns compact `name (file:line)` entries — fast and token-efficient.

---

### Understand a symbol fully

```
explain("ReciprocalRankFusion")
explain("IndexingPipeline", verbosity="summary")
```

Returns the definition, callers, callees, and related code — all in one call. Much cheaper than reading the file.

---

### Check what breaks before you change something

```
impact("TokenBudget")
impact("BaseEmbeddingService", max_depth=5)
```

Runs a transitive caller analysis. Shows every function that would be affected if you change this symbol. Always do this before renaming, deleting, or changing a function signature.

---

### AST-based code operations

The `code` tool gives you structural code intelligence:

```python
# List all symbols in a file
code(operation="get_document_symbols", file_path="src/server.py")

# Fuzzy symbol search across the codebase
code(operation="search_symbols", symbol_name="auth")

# Batch lookup with source code (one call instead of multiple find_symbol calls)
code(operation="lookup_symbols", symbols="AuthService,verify_token", include_source=True)

# Find code by structure (ast-grep patterns)
code(operation="pattern_search", pattern="def $F(self, $$$):", language="python")

# Rewrite code structurally — always preview first
code(operation="pattern_rewrite",
     pattern="logger.info($MSG)",
     replacement="logger.debug($MSG)",
     language="python",
     file_path="src/server.py",   # or path="src/" to rewrite a whole directory
     dry_run=True)                 # set dry_run=False to apply

# Explore a directory's structure
code(operation="search_codebase_map", path="src/auth")
```

---

### Understand project structure

```
overview()        ← file count, languages, top directories, symbol counts
architecture()    ← layers, entry points, hub symbols (most-connected classes)
analyze()         ← code smells, complexity, quality score
analyze(path="src/auth")   ← scoped to a directory
```

---

### Search git history

```
commit_search("when was the payment flow refactored")
commit_search("auth changes", author="alice")
commit_history(limit=10)
commit_history(since="2 weeks ago")
```

Finds commits by meaning, not just keywords.

---

### Remember things across sessions

```
remember("We use JWT with 24h expiry, refresh tokens in Redis")
remember("Decision: use potion-code-16m for all embeddings", memory_type="decision")
recall("JWT token expiry")
forget(tags="outdated")
```

Memories persist across sessions with optional TTL (`day`, `week`, `month`, `permanent`).

---

### Index your own docs and notes

```
knowledge(command="add", name="API docs", value="/path/to/docs/")
knowledge(command="search", query="rate limiting headers")
knowledge(command="show")     ← list all indexed knowledge bases
knowledge(command="remove", name="API docs")
knowledge(command="update", context_id="abc123")   ← refresh by ID
```

Index any text, markdown, or code files and search them semantically.

---

### Archive and recover session context

```
compact(content)                          ← archive session content before compaction
recall(query, memory_type="archive")      ← search archived sessions later
session_snapshot()                        ← recover state after context compaction
```

When your agent's context window fills up, `compact` archives key findings and decisions. After compaction, `session_snapshot()` restores awareness of what was done, and `recall(query, memory_type="archive")` searches the archived content.

---

### Work across multiple repos

```
repo_add("/path/to/other-repo")
repo_status()
repo_remove(path="/path/to/other-repo")
```

Search across all registered repos at once.

---

### Retrieve large outputs on demand

```
retrieve("sx_abc12345")
retrieve("sx_abc12345", query="authentication")
```

Tool responses >1200 tokens are automatically sandboxed and return a compact preview with `sandbox_ref`. Use `retrieve` to fetch the full result on demand.

---

### Server status and health

```
status()    ← indexed?, chunks, symbols, branch, commits, cache hit rate, memory
health()    ← readiness check (use in automated pipelines)
```

---

### Look up Contextro's own docs

```
introspect(query="what tools are available")
introspect(query="how do I use pattern_search")
```

---

## All 35 Tools at a Glance

| Tool | What it does |
|---|---|
| `index` | Index a codebase (runs in background, auto-indexes git history) |
| `search` | Semantic + keyword + graph hybrid search |
| `code` | AST operations: symbol search, pattern search/rewrite, document symbols |
| `find_symbol` | Find a symbol's definition |
| `find_callers` | Who calls this function? |
| `find_callees` | What does this function call? |
| `explain` | Full symbol explanation: definition + callers + callees + related code |
| `impact` | What breaks if I change this? (transitive caller analysis) |
| `analyze` | Code smells, complexity, quality score |
| `overview` | Project structure: languages, files, directories, symbols |
| `architecture` | Layers, entry points, hub symbols |
| `focus` | Low-token context slice for a single file |
| `dead_code` | Entry-point reachability dead-code analysis |
| `circular_dependencies` | SCC-based circular dependency detection |
| `test_coverage_map` | Static test coverage map |
| `audit` | Packaged audit report |
| `commit_search` | Semantic search over git commit history |
| `commit_history` | Browse recent commits |
| `repo_add` | Register another repo for unified search |
| `repo_remove` | Unregister a repo |
| `repo_status` | View all repos and watcher status |
| `remember` | Store a note or decision with tags and TTL |
| `recall` | Search memories (and compaction archive) by meaning |
| `forget` | Delete memories |
| `knowledge` | Index and search your own docs/notes/files |
| `compact` | Archive session content before compaction |
| `session_snapshot` | Compressed session state for context recovery |
| `restore` | Project re-entry summary after a break |
| `docs_bundle` | Generate packaged documentation |
| `sidecar_export` | Generate/clean file-adjacent `.graph.*` sidecars |
| `skill_prompt` | Print or update the agent bootstrap block |
| `introspect` | Look up Contextro's own tool docs and settings |
| `retrieve` | Fetch sandboxed large output by reference ID |
| `status` | Server status, index stats, cache hit rate |
| `health` | Readiness check |

---

## How Search Works

When you call `search("how does auth work")`, Contextro:

1. Checks the query cache for a semantic or exact hit
2. Detects degenerate retrievers (all-equal scores) and zeroes their weight
3. Runs vector search (semantic), BM25 (keyword), and graph search (connectivity) in parallel
4. Boosts BM25 results where the query exactly matches a docstring (2× rank boost)
5. Fuses results with entropy-adaptive Reciprocal Rank Fusion
6. Optionally reranks with FlashRank
7. Filters low-relevance results (threshold: 40% of top score)
8. Applies diversity penalty (no 5 results from the same file)
9. Compresses code snippets with AST-aware compression
10. Returns confidence level (`high`/`medium`/`low`) and `sandbox_ref` if needed

Every step is designed to balance relevance, speed, and token efficiency.

---

## Performance

| Metric | Value |
|---|---|
| Indexing speed | 3,349 files in 6.8s |
| Incremental reindex | 22ms (no changes) |
| Search latency | <2ms (warm index) |
| File discovery | 15ms for 3,349 files |
| Hybrid MRR | 1.000 (20-query benchmark, perfect) |
| tokens_per_search | 116 (down from 378 baseline) |
| tokens_per_explain | 43 (down from 229 baseline) |
| tokens_per_find_callers | 6 (down from 16 baseline) |
| Total workflow tokens | 1,043 (16 tool calls, -59% from baseline) |
| Memory usage | <350MB |

---

## Configuration

All settings are environment variables with the `CTX_` prefix. Most users don't need to change anything — the defaults are optimized for the best balance of speed and quality.

### Common settings

| Variable | Default | What it does |
|---|---|---|
| `CTX_STORAGE_DIR` | `~/.contextro` | Where the index is stored |
| `CTX_EMBEDDING_MODEL` | `potion-code-16m` | Embedding model (see below) |
| `CTX_AUTO_WARM_START` | `false` | Restore index on restart without re-indexing |
| `CTX_RELEVANCE_THRESHOLD` | `0.40` | How strict search filtering is (0–1) |
| `CTX_SEARCH_MODE` | `hybrid` | `hybrid`, `vector`, or `bm25` |
| `CTX_MAX_MEMORY_MB` | `350` | RAM budget |
| `CTX_COMMIT_HISTORY_ENABLED` | `true` | Index git commits for `commit_search` |
| `CTX_LOG_LEVEL` | `INFO` | `DEBUG`, `INFO`, `WARNING`, `ERROR` |
| `CTX_TRANSPORT` | `stdio` | `stdio` (local) or `http` (Docker/remote) |

### Search tuning

| Variable | Default | What it does |
|---|---|---|
| `CTX_SEARCH_CACHE_MAX_SIZE` | `128` | Max cached search responses |
| `CTX_SEARCH_CACHE_TTL_SECONDS` | `300` | Cache expiry (seconds) |
| `CTX_SEARCH_SANDBOX_THRESHOLD_TOKENS` | `1200` | Sandbox responses above this size |
| `CTX_SEARCH_SANDBOX_TTL_SECONDS` | `600` | Sandbox expiry (seconds) |
| `CTX_SEARCH_PREVIEW_RESULTS` | `4` | Preview results when sandboxing |
| `CTX_SEARCH_PREVIEW_CODE_CHARS` | `200` | Code preview length |

### Indexing tuning

| Variable | Default | What it does |
|---|---|---|
| `CTX_CHUNK_CONTEXT_MODE` | `rich` | Chunk header style: `minimal` or `rich` |
| `CTX_SMART_CHUNK_RELATIONSHIPS_ENABLED` | `true` | Index caller→callee relationship chunks |
| `CTX_SMART_CHUNK_FILE_CONTEXT_ENABLED` | `true` | Index file-overview chunks |
| `CTX_COMMIT_HISTORY_LIMIT` | `500` | Max commits to index |
| `CTX_REALTIME_INDEXING_ENABLED` | `true` | Auto-reindex on branch switch |

### Embedding models

| Model | Speed | Quality | Best for |
|---|---|---|---|
| `potion-code-16m` ⭐ | 55k/sec | 99% of SOTA | Daily coding — best balance |
| `potion-8m` | 80k/sec | Good | Maximum speed |
| `jina-code` | 15/sec | Best | Small projects, max precision |
| `nomic-embed` | 15/sec | Good | Docs and markdown |
| `bge-small-en` | 22/sec | OK | Legacy use |

The default (`potion-code-16m`) is trained specifically on code and runs at 55,000 embeddings/sec — fast enough to reindex on every branch switch.

---

## Docker (Team / Server Use)

If you want to run Contextro on a server and share it across a team:

```yaml
# docker-compose.yml
services:
  contextro:
    container_name: contextro-mcp
    image: ghcr.io/jassskalkat/contextro-mcp:latest
    ports:
      - "8000:8000"
    volumes:
      - contextro-data:/data
      - ${CTX_CODEBASE_HOST_PATH}:/repos/platform:ro
    environment:
      CTX_STORAGE_DIR: /data/.contextro
      CTX_CODEBASE_HOST_PATH: ${CTX_CODEBASE_HOST_PATH}
      CTX_CODEBASE_MOUNT_PATH: /repos/platform
      CTX_TRANSPORT: http
      CTX_HTTP_HOST: 0.0.0.0
      CTX_HTTP_PORT: "8000"
      CTX_AUTO_WARM_START: "true"

volumes:
  contextro-data:
```

```bash
export CTX_CODEBASE_HOST_PATH=/path/to/your/project
docker compose up -d
```

The HTTP server auto-enables warm-start so the index is immediately available after restart without calling `index()` again.

```bash
docker pull ghcr.io/jassskalkat/contextro-mcp:latest
```

---

## License

MIT — see [LICENSE](LICENSE) for details.
