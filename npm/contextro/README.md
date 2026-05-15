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
2. Wait a few seconds (your agent will poll status automatically)
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

### Check what breaks before you refactor

```
impact("TokenBudget")
impact("BaseEmbeddingService", max_depth=5)
```

### AST-based code operations

```
code(operation="get_document_symbols", file_path="src/server.rs")
code(operation="search_symbols", symbol_name="auth")
code(operation="pattern_search", pattern="fn $F($$$) -> Result", language="rust")
code(operation="pattern_rewrite", pattern="console.log($MSG)", replacement="logger.info($MSG)", dry_run=True)
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
| `docs_bundle` | Generate documentation from the current index |
| `sidecar_export` | Generate .graph.* sidecars |
| `skill_prompt` | Agent bootstrap block |
| `introspect` | Look up Contextro docs |
| `retrieve` | Fetch archived session content |
| `status` | Server status |
| `health` | Readiness check |
| `refactor_check` | Pre-refactor analysis |

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
