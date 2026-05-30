# contextro

**Give your AI coding agent a brain.**

Contextro is a local [MCP](https://modelcontextprotocol.io) server that connects your AI agent to your codebase. Instead of reading files and guessing, your agent can search by meaning, trace call graphs, check what breaks before a refactor, search git history, and remember context across sessions — all running locally on your machine.

No cloud. No API keys. No data leaves your machine.

---

## Install

```bash
npm install -g contextro
```

Or use it without installing via `npx`:

```bash
npx contextro@latest
```

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
  "mcpServers": {
    "contextro": {
      "command": "npx",
      "args": ["-y", "contextro@latest"]
    }
  }
}
```

The `npx` form always runs the latest version with zero setup — useful for shared team configs.

---

## Getting Started

```
1. Tell your agent: "Index this project at /path/to/your/project"
2. Wait for indexing to finish (some clients poll status automatically)
3. Ask anything about your code
```

The index persists on disk — you only need to do this once per project.

---

## What Your Agent Can Do

### Search your codebase by meaning

```
search("how does authentication work")
search("database connection pool", language="python")
search("TokenBudget", mode="bm25")
```

### Find any symbol

```
find_symbol(symbol_name="IndexingPipeline")
find_symbol(symbol_name="auth", exact=false)
```

### Trace the call graph

```
find_callers(symbol_name="authenticate")
find_callees(symbol_name="authenticate")
```

### Understand a symbol fully

```
explain(symbol_name="ReciprocalRankFusion")
```

### Check what breaks and verify the refactor

```
refactor_check(symbol_name="BaseEmbeddingService")
impact(symbol_name="TokenBudget", max_depth=5)
completion_check(claim="all_callers_updated", symbol_name="BaseEmbeddingService", changed_files=["src/embeddings.rs", "src/search.rs"])
```

### AST-based code operations

```
code(operation="get_document_symbols", path="src/server.rs")
code(operation="search_symbols", symbol_name="auth")
code(operation="pattern_search", pattern="fn $F($$$) -> Result", language="rust")
code(operation="pattern_rewrite", pattern="console.log($MSG)", replacement="logger.info($MSG)", dry_run=true)
```

`get_document_symbols` returns a compact columnar payload: `{ file, columns, symbols, total }`.
Pass `include_signature=true` only when you need signatures; `list_symbols(path=<file>)` uses the same file contract, while `list_symbols(path=<dir>)` returns object rows with `callers` and `callees`.

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

## All 38 Tools

| Tool | What it does |
|---|---|
| `index` | Index a codebase |
| `search` | Semantic + keyword + graph hybrid search |
| `code` | AST operations: symbol inventories/search, pattern search/rewrite, edit plan, codebase map |
| `find_symbol` | Find a symbol's definition |
| `find_callers` | Who calls this function? |
| `find_callees` | What does this function call? |
| `explain` | Full symbol explanation |
| `impact` | What breaks if I change this? |
| `refactor_check` | Pre-refactor analysis |
| `completion_check` | Verify that a rename/signature change really updated all callers |
| `analyze` | Code smells, complexity |
| `overview` | Project structure |
| `architecture` | Hub symbols, layers |
| `focus` | Low-token context slice |
| `dead_code` | Unreachable code detection |
| `circular_dependencies` | SCC-based cycle detection |
| `test_coverage_map` | Static test coverage bounds |
| `audit` | Packaged audit report |
| `commit_search` | Semantic git history search |
| `commit_history` | Browse recent commits |
| `repo_add` | Register another repo for later indexing/search |
| `repo_remove` | Unregister a repo |
| `repo_status` | View registered repos |
| `remember` | Store a note/decision |
| `recall` | Search memories |
| `forget` | Delete memories |
| `tags` | List all memory tags |
| `knowledge` | Index and search docs |
| `compact` | Archive session content |
| `session_snapshot` | Recent session context |
| `restore` | Project re-entry summary |
| `docs_bundle` | Generate documentation from the current index |
| `sidecar_export` | Generate .graph.* sidecars |
| `skill_prompt` | Agent bootstrap block |
| `introspect` | Look up Contextro docs |
| `retrieve` | Fetch archived session content |
| `status` | Server status + active repo |
| `health` | Readiness check |

---

## Why Contextro?

Without Contextro, your agent reads 5–10 full files to find one function. With Contextro, it finds the exact chunk in one search call.

```
Without:  grep "auth" → read auth.py → read middleware.py → read utils.py → ...
With:     search("authentication flow") → exact result in <1ms
```

| Metric | Baseline | Contextro | Improvement |
|---|---|---|---|
| Success rate | 99.5% | **100%** | +0.5% |
| Total tokens | 941,748 | **93,819** | **90% reduction** |
| Median latency | 199.8ms | **0.081ms** | **2,466x faster** |
| Tool calls per task | 3.2 | **1.0** | 68% fewer |
| Files read | 1,961 | **0** | Eliminated |

---

## Configuration

All settings via environment variables:

| Variable | Default | What it does |
|---|---|---|
| `CTX_STORAGE_DIR` | `~/.contextro` | Where the index is stored |
| `CTX_TRANSPORT` | `stdio` | `stdio` or `http` |
| `CTX_HTTP_HOST` | `0.0.0.0` | HTTP bind address (http mode) |
| `CTX_HTTP_PORT` | `8000` | HTTP port (http mode) |
| `CTX_LOG_LEVEL` | `INFO` | Logging level |

---

## Supported Platforms

| Platform | Architecture |
|---|---|
| macOS | Apple Silicon (M1/M2/M3), Intel |
| Linux | x86_64, ARM64 |
| Windows | x86_64 |

---

## License

Source-available under the Business Source License 1.1 (`BUSL-1.1`).
Internal production use is permitted under the Additional Use Grant in
`LICENSE`. This version converts to Apache License 2.0 on 2030-05-11, or on
the fourth anniversary of its first public release, whichever comes first.
