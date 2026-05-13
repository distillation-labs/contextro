# Contextro

**Give your AI coding agent a brain.**

Contextro is a local MCP server that connects your AI agent (Claude, Cursor, Windsurf, etc.) to your codebase. Instead of reading files and guessing, your agent can search by meaning, trace call graphs, check what breaks before a refactor, search git history, and remember context across sessions — all running locally on your machine.

No cloud. No API keys. No data leaves your machine.

---

## Why Contextro?

Without Contextro, your agent reads 5–10 full files to find one function. With Contextro, it finds the exact chunk in one search call.

```
Without:  grep "auth" → read auth.py → read middleware.py → read utils.py → ...
With:     search("authentication flow") → exact result in <0.1ms
```

| Task | Without Contextro | With Contextro | Savings |
|---|---|---|---|
| Find a function | Read 5 files (~1000 tokens) | `search()` (~94 tokens) | **11x** |
| Trace callers | grep + read 3 files (~3000 tokens) | `find_callers()` (~50 tokens) | **60x** |
| Understand a class | Read file + grep (~2000 tokens) | `explain()` (~170 tokens) | **12x** |
| Check what breaks | Manual audit (~8000 tokens) | `impact()` (~108 tokens) | **74x** |

### Benchmark (1,000-task study on a production TypeScript monorepo)

| Metric | Baseline (git grep + file reads) | Contextro | Improvement |
|---|---|---|---|
| Success rate | 99.5% | **100%** | +0.5% |
| Total tokens | 941,748 | **93,819** | **90% reduction** |
| Median latency | 199.8ms | **0.081ms** | **2,466x faster** |
| Tool calls per task | 3.2 | **1.0** | 68% fewer |
| Files read | 1,961 | **0** | Eliminated |

### What powers it

- **Real tree-sitter parsing** — TypeScript, JavaScript, Python parsed via tree-sitter AST (not regex heuristics)
- **BM25 + vector hybrid search** — Tantivy full-text with potion-code-16M embeddings
- **PageRank-weighted call graph** — importance propagates transitively through the graph
- **Graph consensus boosting** — architecturally-related results surface together
- **Persistent index** — survives server restarts, stored at `~/.contextro/projects/`

---

## Install

```bash
npm install -g contextro
```

Single binary. No runtime dependencies. No setup.

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
search("TokenBudget", mode="bm25")
```

### Find any symbol

```
find_symbol("IndexingPipeline")
find_symbol("auth", exact=False)
```

### Trace the call graph

```
find_callers("authenticate")
find_callees("authenticate")
```

### Understand a symbol fully

```
explain("ReciprocalRankFusion")
```

### Check what breaks

```
impact("TokenBudget")
impact("BaseEmbeddingService", max_depth=5)
```

### AST-based code operations

```
code(operation="get_document_symbols", file_path="src/server.rs")
code(operation="search_symbols", symbol_name="auth")
code(operation="pattern_search", pattern="fn $F($$$) -> Result", language="rust")
code(operation="pattern_rewrite", pattern="println!($MSG)", replacement="tracing::info!($MSG)", dry_run=True)
code(operation="edit_plan", goal="Replace legacy logging", symbol_name="log_event")
```

### Search git history

```
commit_search("when was the payment flow refactored")
commit_history(limit=10)
```

### Remember things across sessions

```
remember("We use JWT with 24h expiry, refresh tokens in Redis")
recall("JWT token expiry")
forget(tags="outdated")
```

### Index your own docs

```
knowledge(command="add", name="API docs", value="/path/to/docs/")
knowledge(command="search", query="rate limiting")
```

### Analysis tools

```
dead_code()
circular_dependencies()
test_coverage_map()
focus(path="src/auth.rs")
```

---

## All 37 Tools

| Tool | What it does |
|---|---|
| `index` | Index a codebase |
| `search` | Semantic + keyword + graph hybrid search |
| `code` | AST operations: symbol search, pattern search/rewrite, edit plan |
| `find_symbol` | Find a symbol's definition |
| `find_callers` | Who calls this function? |
| `find_callees` | What does this function call? |
| `explain` | Full symbol explanation |
| `impact` | What breaks if I change this? |
| `refactor_check` | Pre-refactor analysis: callers + callees + impact + risk in one call |
| `analyze` | Code smells, complexity |
| `overview` | Project structure |
| `architecture` | Hub symbols, layers |
| `focus` | Low-token context slice |
| `dead_code` | Unreachable code detection |
| `circular_dependencies` | Import cycle detection (Rust + TypeScript) |
| `test_coverage_map` | Static test coverage |
| `audit` | Packaged audit report |
| `commit_search` | Semantic git history search |
| `commit_history` | Browse recent commits |
| `repo_add` | Register another repo |
| `repo_remove` | Unregister a repo |
| `repo_status` | View all repos |
| `remember` | Store a note/decision |
| `recall` | Search memories |
| `forget` | Delete memories |
| `tags` | List all memory tags |
| `knowledge` | Index and search docs |
| `compact` | Archive session content |
| `session_snapshot` | Context recovery |
| `restore` | Project re-entry summary |
| `docs_bundle` | Generate documentation |
| `sidecar_export` | Generate .graph.* sidecars |
| `skill_prompt` | Agent bootstrap block |
| `introspect` | Look up Contextro docs |
| `retrieve` | Fetch sandboxed output |
| `status` | Server status |
| `health` | Readiness check |

---

## Configuration

All settings via `CTX_` environment variables:

| Variable | Default | What it does |
|---|---|---|
| `CTX_STORAGE_DIR` | `~/.contextro` | Where the index is stored |
| `CTX_TRANSPORT` | `stdio` | `stdio` or `http` |
| `CTX_HTTP_HOST` | `0.0.0.0` | HTTP bind address |
| `CTX_HTTP_PORT` | `8000` | HTTP port |
| `CTX_LOG_LEVEL` | `INFO` | Logging level |
| `CTX_EMBEDDING_MODEL` | `potion-code-16m` | Embedding model for vector search |
| `CTX_TOOL_TIER` | `full` | `core` (10 tools), `standard` (22), or `full` (36) |
| `CTX_NO_UPDATE_CHECK` | unset | Set to `1` to disable update checks |
| `CTX_EMBEDDING_MODEL` | `potion-code-16m` | Embedding model for vector search |
| `CTX_TOOL_TIER` | `full` | `core` (10 tools), `standard` (22), or `full` (36) |
| `CTX_NO_UPDATE_CHECK` | unset | Set to `1` to disable update checks |

---

## Docker (Team / Server Use)

```yaml
services:
  contextro:
    image: ghcr.io/distillation-labs/contextro-mcp:latest
    ports:
      - "8000:8000"
    volumes:
      - contextro-data:/data
      - ${CTX_CODEBASE_HOST_PATH}:/repos/platform:ro
    environment:
      CTX_STORAGE_DIR: /data/.contextro
      CTX_TRANSPORT: http
      CTX_HTTP_HOST: 0.0.0.0
      CTX_HTTP_PORT: "8000"
```

---

## License

Source-available under the Business Source License 1.1 (`BUSL-1.1`).
Production use is limited by the Additional Use Grant in [LICENSE](LICENSE).
This version converts to Apache License 2.0 on 2030-05-11, or on the fourth
anniversary of its first public release, whichever comes first.
