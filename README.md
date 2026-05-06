# Contextia

**Give your AI coding agent a brain.**

Contextia is a local MCP server that connects your AI agent (Claude, Cursor, Windsurf, etc.) to your codebase. Instead of reading files and guessing, your agent can search by meaning, trace call graphs, check what breaks before a refactor, search git history, and remember context across sessions — all running locally on your machine.

No cloud. No API keys. No data leaves your machine.

---

## Why Contextia?

Without Contextia, your agent reads 5–10 full files to find one function. With Contextia, it finds the exact chunk in one search call.

```
Without:  grep "auth" → read auth.py → read middleware.py → read utils.py → ...
With:     search("authentication flow") → exact result in <2ms
```

That's roughly a **9x reduction in tokens** per session.

---

## Install

```bash
pip install contextia
```

**Requirements:** Python 3.10–3.12

Optional extras:
```bash
pip install contextia[reranker]   # Better search quality (FlashRank)
pip install contextia[model2vec]  # Fast embeddings (55k/sec)
```

---

## Connect to Your Agent

### Claude Code
```bash
claude mcp add contextia -- contextia
```

### Claude Desktop
Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "contextia": {
      "command": "contextia"
    }
  }
}
```

### Cursor / Windsurf / Any MCP client
```json
{
  "contextia": {
    "command": "contextia",
    "transport": "stdio"
  }
}
```

---

## First Use

```
1. Tell your agent: "Index this project at /path/to/your/project"
2. Wait a few seconds (agent will poll status automatically)
3. Ask anything about your code
```

That's it. The index persists on disk — you only need to do this once per project.

---

## What You Can Do

### Search your codebase

```
search("how does authentication work")
search("database connection pool", language="python")
search("TokenBudget", mode="bm25")   ← exact keyword match
```

Results include the code snippet, file, line number, per-result `match` hints, and a top-level `confidence` field. If the payload gets too large, search returns an inline preview plus `sandbox_ref` so the full result set can be fetched on demand.

---

### Find any symbol

```
find_symbol("IndexingPipeline")           ← exact match
find_symbol("auth", exact=False)          ← fuzzy search
```

Returns the definition location, caller count, and top callers.

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

Runs a transitive caller analysis. Always do this before renaming, deleting, or changing a function signature.

---

### AST-based code operations

The `code` tool gives you structural code intelligence:

```
# List all symbols in a file
code(operation="get_document_symbols", file_path="src/server.py")

# Fuzzy symbol search
code(operation="search_symbols", symbol_name="auth")

# Batch lookup with source code
code(operation="lookup_symbols", symbols="AuthService,verify_token")

# Find code by structure (ast-grep patterns)
code(operation="pattern_search", pattern="def $F(self, $$$):", language="python")

# Rewrite code structurally — preview first
code(operation="pattern_rewrite",
     pattern="logger.info($MSG)",
     replacement="logger.debug($MSG)",
     language="python",
     file_path="src/server.py",
     dry_run=True)    ← set dry_run=False to apply

# Explore a directory
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
```

Index any text, markdown, or code files and search them semantically.

---

### Recover after context compaction

```
session_snapshot()    ← always call this first after compaction
```

Returns what you were working on: recent searches, symbols explored, key actions, codebase path, branch, and chunk count.

---

### Work across multiple repos

```
repo_add("/path/to/other-repo")
repo_status()
repo_remove(path="/path/to/other-repo")
```

Search across all registered repos at once.

---

### Server status and health

```
status()    ← indexed?, chunks, symbols, branch, commits, cache hit rate, memory
health()    ← readiness check (use in automated pipelines)
```

---

### Look up Contextia's own docs

```
introspect(query="what tools are available")
introspect(query="how do I use pattern_search")
```

---

### Retrieve large outputs on demand

```
retrieve("sx_abc12345")
retrieve("sx_abc12345", query="authentication")
```

When a tool result is very large, Contextia stores it in a sandbox and returns a reference ID. Use `retrieve` to fetch it when you need it. Search now pairs this with `full_total` so agents can keep working from the preview without paying the full token cost up front.

---

## All 25 Tools at a Glance

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
| `commit_search` | Semantic search over git commit history |
| `commit_history` | Browse recent commits |
| `repo_add` | Register another repo for unified search |
| `repo_remove` | Unregister a repo |
| `repo_status` | View all repos and watcher status |
| `remember` | Store a note or decision with tags and TTL |
| `recall` | Search memories by meaning |
| `forget` | Delete memories |
| `knowledge` | Index and search your own docs/notes/files |
| `session_snapshot` | Compressed session state for context recovery |
| `introspect` | Look up Contextia's own tool docs and settings |
| `retrieve` | Fetch sandboxed large output by reference ID |
| `status` | Server status, index stats, cache hit rate |
| `health` | Readiness check |

---

## Docker (for teams or remote use)

If you want to run Contextia on a server and share it across a team:

```yaml
# docker-compose.yml
services:
  contextia:
    image: jassskalkat/contextia-mcp:latest
    ports:
      - "8000:8000"
    volumes:
      - contextia-data:/data
      - ${CTX_CODEBASE_HOST_PATH}:/repos/platform:ro
    environment:
      CTX_STORAGE_DIR: /data/.contextia
      CTX_PATH_PREFIX_MAP: ${CTX_PATH_PREFIX_MAP}
      CTX_TRANSPORT: http
      CTX_HTTP_HOST: 0.0.0.0
      CTX_HTTP_PORT: "8000"
      CTX_AUTO_WARM_START: "true"
      CTX_COMMIT_HISTORY_ENABLED: "true"
      CTX_FILE_WATCHER_ENABLED: "false"
      CTX_SEARCH_SANDBOX_THRESHOLD_TOKENS: "1200"
      CTX_SEARCH_PREVIEW_RESULTS: "4"
      CTX_SEARCH_PREVIEW_CODE_CHARS: "220"

volumes:
  contextia-data:
```

```bash
# Set your repo path and start
export CTX_CODEBASE_HOST_PATH=/Users/you/myproject
export CTX_PATH_PREFIX_MAP="/Users/you/myproject=/repos/platform"
docker compose up -d
```

The shipped image is now multi-stage: build-only compilers stay out of the runtime layer, the virtualenv is copied forward, and the default Model2Vec embedding is pre-cached for predictable cold starts.

---

## Configuration

All settings are environment variables with the `CTX_` prefix:

| Variable | Default | What it does |
|---|---|---|
| `CTX_STORAGE_DIR` | `~/.contextia` | Where the index is stored |
| `CTX_EMBEDDING_MODEL` | `potion-code-16m` | Embedding model (see below) |
| `CTX_AUTO_WARM_START` | `false` | Restore index on restart without re-indexing |
| `CTX_RELEVANCE_THRESHOLD` | `0.40` | How strict search filtering is (0–1) |
| `CTX_SEARCH_MODE` | `hybrid` | `hybrid`, `vector`, or `bm25` |
| `CTX_SEARCH_SANDBOX_THRESHOLD_TOKENS` | `1200` | Sandbox oversized search responses above this token estimate |
| `CTX_SEARCH_PREVIEW_RESULTS` | `4` | How many preview results stay inline when search sandboxes output |
| `CTX_SEARCH_PREVIEW_CODE_CHARS` | `220` | Code preview length for sandboxed search hits |
| `CTX_MAX_MEMORY_MB` | `350` | RAM budget |
| `CTX_COMMIT_HISTORY_ENABLED` | `true` | Index git commits for `commit_search` |
| `CTX_COMMIT_HISTORY_LIMIT` | `500` | Max commits to index |
| `CTX_REALTIME_INDEXING_ENABLED` | `true` | Auto-reindex on branch switch |
| `CTX_LOG_LEVEL` | `INFO` | `DEBUG`, `INFO`, `WARNING`, `ERROR` |
| `CTX_TRANSPORT` | `stdio` | `stdio` (local) or `http` (Docker/remote) |

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

## How Search Works (briefly)

When you call `search("how does auth work")`, Contextia:

1. Routes the request through the shared execution engine in `execution/search.py`
2. Checks the query cache, namespaced by search options, for an exact or semantic hit
3. Runs vector search (semantic similarity), BM25 (keyword), and graph search (connectivity) in parallel
4. Fuses results with RRF (Reciprocal Rank Fusion)
5. Optionally reranks with FlashRank
6. Filters out low-relevance results (threshold: 40% of top score)
7. Applies a diversity penalty so you don't get 5 results from the same file
8. Compresses snippets, applies any inline context budget, and sandboxes oversized payloads
9. Returns a `confidence` field (`high`/`medium`/`low`), token count, and `sandbox_ref` when needed

Every step is backed by published research. See [CONTRIBUTING.md](CONTRIBUTING.md) for citations.

---

## Development

```bash
git clone https://github.com/jassskalkat/Contextia-MCP.git
cd Contextia-MCP
python3.12 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev,reranker,model2vec]"

pytest -v
ruff check .
python scripts/benchmark_token_efficiency.py
python scripts/benchmark_retrieval_quality.py --path src --query-limit 20
BROWSER_USE_PATH=/path/to/browser-use python scripts/benchmark_browser_use.py
contextia          # run the server
```

When changing search/snippet compression, capture the token-efficiency benchmark and the retrieval-quality scorecard before and after with the same query limit so token savings never come at the cost of Recall@K / MRR quality regressions.

See [CONTRIBUTING.md](CONTRIBUTING.md) for architecture details, how to add tools, and PR guidelines.

---

## License

MIT — see [LICENSE](LICENSE) for details.
