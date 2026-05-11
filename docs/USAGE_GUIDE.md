# Usage Guide

## Getting Started

Install or build the `contextro` binary, then run it through your MCP client.

```bash
contextro
```

The default transport is stdio. For HTTP mode:

```bash
CTX_TRANSPORT=http contextro
```

The HTTP server exposes:

- `GET /health`
- `POST /mcp`

## MCP Client Configuration

Claude Code:

```bash
claude mcp add contextro -- contextro
```

Claude Desktop:

```json
{
  "mcpServers": {
    "contextro": {
      "command": "contextro"
    }
  }
}
```

Other MCP clients:

```json
{
  "contextro": {
    "command": "contextro",
    "transport": "stdio"
  }
}
```

## Typical Session

1. Start Contextro through your MCP client.
2. Run `index(path="/path/to/project")`.
3. Use search, graph, and analysis tools against the indexed project.
4. Re-run `index` after significant code changes.

## Tool Reference

### Required First Step: `index`

Index a codebase before using search and graph tools.

```text
index(path="/path/to/your/project")
```

Returns indexing statistics including file count, symbol count, chunk count, graph size, and elapsed time.

### Search and Navigation

`search`

```text
search(query="authentication middleware", limit=10, mode="hybrid")
```

Hybrid retrieval combines semantic search, BM25, and graph signals.

`find_symbol`

```text
find_symbol(name="IndexingPipeline", exact=true)
```

`find_callers`

```text
find_callers(symbol_name="authenticate")
```

`find_callees`

```text
find_callees(symbol_name="authenticate")
```

`explain`

```text
explain(symbol_name="ReciprocalRankFusion")
```

`impact`

```text
impact(symbol_name="TokenBudget", max_depth=5)
```

### Project-Level Analysis

`status`

```text
status()
```

`health`

```text
health()
```

`overview`

```text
overview()
```

`architecture`

```text
architecture()
```

`analyze`

```text
analyze(path="src/auth")
```

`focus`

```text
focus(path="src/auth.rs")
```

`dead_code`, `circular_dependencies`, and `test_coverage_map` provide additional static analysis views once the codebase is indexed.

### Code-Aware Operations

The `code` tool exposes AST-oriented operations.

```text
code(operation="get_document_symbols", file_path="src/main.rs")
code(operation="search_symbols", symbol_name="auth")
code(operation="pattern_search", pattern="fn $F($$$) -> Result", path="src")
```

### Memory and Session Tools

```text
remember(content="JWT expires after 24h", memory_type="decision", tags="auth,jwt")
recall(query="JWT expiry")
forget(tags="temporary")
session_snapshot()
restore()
compact(content="Finished indexing and auth investigation")
retrieve(ref_id="...")
```

### Git and Multi-Repo Tools

```text
commit_search(query="when was the payment flow refactored")
commit_history(limit=10)
repo_add(path="/path/to/another/repo")
repo_status()
```

## Configuration

Common runtime settings:

```bash
export CTX_STORAGE_DIR="$HOME/.contextro"
export CTX_EMBEDDING_MODEL=potion-code-16m
export CTX_SEARCH_MODE=hybrid
export CTX_LOG_LEVEL=INFO
export CTX_TRANSPORT=stdio
```

For HTTP deployments:

```bash
export CTX_TRANSPORT=http
export CTX_HTTP_HOST=0.0.0.0
export CTX_HTTP_PORT=8000
```

## Best Practices

1. Index the project root, not an arbitrary subdirectory.
2. Start with `search` or `find_symbol` before manually reading files.
3. Use `impact` before changing shared symbols.
4. Use `overview` and `architecture` when starting in an unfamiliar repository.
5. Keep Contextro running as the `contextro` binary rather than wrapping it in another runtime.

## Language Support

Contextro parses many languages through tree-sitter, including Rust, TypeScript, JavaScript, Go, Java, C, C++, and Python source. Python is a supported target language for indexing and search, not a runtime dependency.
