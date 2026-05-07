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

## Development HTTP Loop

Use `docker-compose.dev.yml` for the fast edit/test loop. It bind-mounts `src/` and `scripts/`, runs a small supervisor that watches for source changes, and restarts the HTTP MCP process automatically without rebuilding the image.

```bash
# Point Contextia at the repo you want it to index
export CTX_CODEBASE_HOST_PATH=/Users/you/myproject

# Start the live-reloading dev server
docker compose -f docker-compose.dev.yml up --build
```

What this gives you:

- code changes under `src/` and `scripts/` restart the MCP server automatically
- the MCP URL stays stable at `http://localhost:8000/mcp`
- `CTX_AUTO_WARM_START=true` restores the previous index after each restart
- you only need a rebuild when dependencies or the Docker image itself change

Important:

- behavior changes can usually be tested immediately after the server restarts
- tool schema changes may still require the MCP client to reconnect and refresh tool definitions

---

## Docker (stable image)

If you want to run Contextia on a server and share it across a team:

```yaml
# docker-compose.yml
services:
  contextia:
    container_name: contextia-mcp
    image: ghcr.io/jassskalkat/contextia-mcp:latest
    ports:
      - "8000:8000"
    volumes:
      - contextia-data:/data
      - ${CTX_CODEBASE_HOST_PATH}:/repos/platform:ro
    environment:
      CTX_STORAGE_DIR: /data/.contextia
      CTX_CODEBASE_HOST_PATH: ${CTX_CODEBASE_HOST_PATH}
      CTX_CODEBASE_MOUNT_PATH: /repos/platform
      CTX_PATH_PREFIX_MAP: ${CTX_PATH_PREFIX_MAP:-}
      CTX_TRANSPORT: http
      CTX_HTTP_HOST: 0.0.0.0
      CTX_HTTP_PORT: "8000"
      CTX_AUTO_WARM_START: "true"
      CTX_COMMIT_HISTORY_ENABLED: "true"
      CTX_FILE_WATCHER_ENABLED: "false"
      CTX_SEARCH_CACHE_TTL_SECONDS: "300"
      CTX_SEARCH_SANDBOX_TTL_SECONDS: "600"
      CTX_SEARCH_SANDBOX_THRESHOLD_TOKENS: "1200"
      CTX_SEARCH_PREVIEW_RESULTS: "4"
      CTX_SEARCH_PREVIEW_CODE_CHARS: "220"

volumes:
  contextia-data:
```

```bash
# Set your repo path and start
export CTX_CODEBASE_HOST_PATH=/Users/you/myproject
docker compose up -d
```

Contextia now auto-remaps that host path to `/repos/platform` inside the container. Set
`CTX_PATH_PREFIX_MAP` only if your MCP client sends a different host-path alias than
`CTX_CODEBASE_HOST_PATH`.

Pull the published image directly with:

```bash
docker pull ghcr.io/jassskalkat/contextia-mcp:latest
```

The shipped image is multi-stage: build-only compilers stay out of the runtime layer, the virtualenv is copied forward, and the default Model2Vec embedding is pre-cached for predictable cold starts.

---

## Alpha Channel

The repo now supports a separate alpha deployment loop for live MCP validation without touching `latest`:

1. Push changes to the `alpha` branch.
2. GitHub Actions runs lint + tests.
3. It publishes:
   - `ghcr.io/<owner>/contextia-mcp:alpha`
   - `ghcr.io/<owner>/contextia-mcp:alpha-<short-sha>`
4. If SSH deployment secrets are configured, the workflow also updates a remote alpha host in place.

Required secrets for automatic remote alpha deploy:

- `ALPHA_SSH_HOST`
- `ALPHA_SSH_USER`
- `ALPHA_SSH_PRIVATE_KEY`

Optional secrets:

- `ALPHA_SSH_PORT` (default `22`)
- `ALPHA_REMOTE_DIR` (default `/opt/contextia-alpha`)
- `ALPHA_HTTP_PORT` (default `8000`)

The alpha deploy uses `deploy/alpha/docker-compose.yml` and defaults to a persistent `/data` volume only. If you want the alpha host to index a fixed external codebase, add a bind mount there or call `index(path="/app/src")` to smoke-test against the code shipped inside the image.

---

## Configuration

All settings are environment variables with the `CTX_` prefix:

| Variable | Default | What it does |
|---|---|---|
| `CTX_STORAGE_DIR` | `~/.contextia` | Where the index is stored |
| `CTX_EMBEDDING_MODEL` | `potion-code-16m` | Embedding model (see below) |
| `CTX_AUTO_WARM_START` | `false` | Restore index on restart without re-indexing |
| `CTX_CHUNK_CONTEXT_MODE` | `rich` | Chunk header style: `minimal` or `rich`. The default `smart` profile uses `rich`. |
| `CTX_CHUNK_CONTEXT_PATH_DEPTH` | `4` | How many path segments to keep in chunk context |
| `CTX_SMART_CHUNK_RELATIONSHIPS_ENABLED` | `true` | Index caller→callee relationship chunks. Enabled in the default `smart` profile. |
| `CTX_SMART_CHUNK_FILE_CONTEXT_ENABLED` | `true` | Index file-overview chunks. Enabled in the default `smart` profile. |
| `CTX_RELEVANCE_THRESHOLD` | `0.40` | How strict search filtering is (0–1) |
| `CTX_SEARCH_MODE` | `hybrid` | `hybrid`, `vector`, or `bm25` |
| `CTX_SEARCH_CACHE_MAX_SIZE` | `128` | Max cached search responses kept per session |
| `CTX_SEARCH_CACHE_SIMILARITY_THRESHOLD` | `0.92` | Semantic cache reuse threshold |
| `CTX_SEARCH_CACHE_TTL_SECONDS` | `300` | Expire stale cached search responses after this many seconds |
| `CTX_SEARCH_SANDBOX_THRESHOLD_TOKENS` | `1200` | Sandbox oversized search responses above this token estimate |
| `CTX_SEARCH_SANDBOX_MAX_ENTRIES` | `100` | Max sandboxed payloads retained in memory |
| `CTX_SEARCH_SANDBOX_TTL_SECONDS` | `600` | Expire stale sandbox payloads after this many seconds |
| `CTX_SEARCH_PREVIEW_RESULTS` | `4` | How many preview results stay inline when search sandboxes output |
| `CTX_SEARCH_PREVIEW_CODE_CHARS` | `220` | Code preview length for sandboxed search hits |
| `CTX_MAX_MEMORY_MB` | `350` | RAM budget |
| `CTX_COMMIT_HISTORY_ENABLED` | `true` | Index git commits for `commit_search` |
| `CTX_COMMIT_HISTORY_LIMIT` | `500` | Max commits to index |
| `CTX_REALTIME_INDEXING_ENABLED` | `true` | Auto-reindex on branch switch |
| `CTX_LOG_LEVEL` | `INFO` | `DEBUG`, `INFO`, `WARNING`, `ERROR` |
| `CTX_TRANSPORT` | `stdio` | `stdio` (local) or `http` (Docker/remote) |

### Chunk profiles

The current default chunking setup is the benchmark-backed `smart` profile:

- `CTX_CHUNK_CONTEXT_MODE=rich`
- `CTX_SMART_CHUNK_RELATIONSHIPS_ENABLED=true`
- `CTX_SMART_CHUNK_FILE_CONTEXT_ENABLED=true`

In the current in-repo benchmark on `src` with `20` generated queries, `smart` kept the best overall retrieval balance:

| Profile | Hybrid MRR | Hybrid Recall@5 | Hybrid Avg Tokens | Vector MRR | Index Chunks |
|---|---:|---:|---:|---:|---:|
| `smart` (default) | `0.625` | `0.9` | `491` | `0.625` | `1223` |
| `contextual` | `0.625` | `0.9` | `510` | `0.533` | `672` |
| `minimal` | `0.55` | `0.55` | `450` | `0.55` | `672` |

Use `minimal` only when smaller index footprint matters more than retrieval quality. Use `contextual` if you want rich symbol headers without the extra relationship and file-overview chunks. Keep `smart` as the default when search quality is the primary goal.

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

The response-assembly policy now lives in `execution/response_policy.py`, which keeps token budgeting,
preview shaping, and sandbox handoff separate from retrieval/ranking logic.

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
docker compose -f docker-compose.dev.yml up --build
python scripts/benchmark_token_efficiency.py
python scripts/benchmark_retrieval_quality.py --path src --query-limit 20
python scripts/benchmark_chunk_profiles.py --path src --query-limit 20
BROWSER_USE_PATH=/path/to/browser-use python scripts/benchmark_browser_use.py
contextia          # run the server
```

When changing search/snippet compression, capture the token-efficiency benchmark and the retrieval-quality scorecard before and after with the same query limit so token savings never come at the cost of Recall@K / MRR quality regressions.

Run the benchmark scripts from the activated project venv with `python`, or use an explicit Python `3.10`-`3.12` interpreter such as `python3.11` or `python3.12`.

Use `docker-compose.dev.yml` for the inner MCP loop and `alpha` branch pushes for the hosted alpha loop. Do not use stable image publishes as the day-to-day development loop.

See [CONTRIBUTING.md](CONTRIBUTING.md) for architecture details, how to add tools, and PR guidelines.

---

## License

MIT — see [LICENSE](LICENSE) for details.
