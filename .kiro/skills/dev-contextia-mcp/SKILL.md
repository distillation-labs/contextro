---
name: dev-contextia-mcp
description: >
  Code intelligence for coding agents via Contextia MCP — semantic search, call graph
  traversal, impact analysis, AST search/rewrite, git history, persistent memory, cross-repo
  search, and session recovery. Use when the user asks to find code, understand a function,
  trace callers, check what breaks before a refactor, search commit history, index a project,
  store context, or recover session state. Trigger phrases: "index this project", "search the
  codebase", "who calls X", "what does X call", "what breaks if I change X", "explain this
  symbol", "find where X is used", "search git history", "what changed recently", "remember
  this", "recall context", "pattern search", "AST search", "find all usages", "add another
  repo", "retrieve result". Always prefer Contextia tools over grep, readFile, and glob —
  they are faster, more accurate, and use up to 9.8x fewer tokens.
metadata:
  version: "3.0.0"
  mcp-server: contextia
  tags: [contextia, mcp, code-search, code-graph, indexing, git, memory, session, ast, cross-repo]
license: MIT
---

# Contextia MCP

Contextia is a fully local code intelligence MCP server. 25 tools. No cloud. No API keys. <350MB RAM.

**Core rule: Always prefer Contextia tools over reading files, grepping, or globbing.**
A single `search` call returns the exact chunk in <2ms. Reading files wastes 50–100x more tokens.

---

## Step 0: Index First

Before any tool call, ensure the codebase is indexed:

```
1. index("/absolute/path/to/project")  → returns immediately with status: indexing
2. status()                             → poll every 3s until indexed: true
3. Use any tool
```

**Never call search/explain/find_callers immediately after index without checking status() first.**
Background indexing takes 10–30s for large codebases.

**After server restart with CTX_AUTO_WARM_START=true (default in Docker):** index restores
automatically — call `status()` to confirm `indexed: true`, skip `index()`.

---

## All 25 Tools

### Indexing & Status

| Tool | When to use | Key params |
|---|---|---|
| `index(path)` | Index a codebase. Call once; index persists on disk. | `path`: absolute dir path; `paths`: additional comma-separated paths |
| `status()` | Check if indexed, get stats (chunks, graph, branch, cache). Poll after `index()`. | — |
| `health()` | Readiness check. Use in automated pipelines before any tool call. | — |

### Search

| Tool | When to use | Key params |
|---|---|---|
| `search(query)` | Find code by meaning, keyword, or structure. Best first tool for any code question. | `query`, `limit` (default 10), `language`, `symbol_type`, `mode` (hybrid/vector/bm25), `rerank` (default true), `context_budget` |
| `find_symbol(name)` | Find a symbol's definition by exact or fuzzy name. | `name`, `exact` (default true) |
| `find_callers(symbol_name)` | Who calls this function? Returns compact `name (file:line)` list. | `symbol_name` |
| `find_callees(symbol_name)` | What does this function call? Returns compact `name (file:line)` list. | `symbol_name` |
| `explain(symbol_name)` | Full symbol context: definition + callers + callees + related code. One call replaces readFile + grep. | `symbol_name`, `verbosity` (summary/detailed/full) |
| `impact(symbol_name)` | Transitive caller analysis. Run before ANY rename/delete/signature change. | `symbol_name`, `max_depth` (default 10) |
| `retrieve(ref_id)` | Fetch a sandboxed large result by reference ID. Use when search returns `sandbox_ref`. | `ref_id` (e.g. `sx_abc12345`), `query` (optional filter) |

### Code Intelligence (AST)

| Tool | When to use | Key params |
|---|---|---|
| `code(operation, ...)` | AST operations: symbol search, pattern search/rewrite, document symbols, codebase map. | See operations below |

**`code` operations:**

```python
# List all symbols in a file
code(operation="get_document_symbols", file_path="src/auth/service.py")

# Fuzzy symbol search across codebase
code(operation="search_symbols", symbol_name="auth", language="python")

# Batch lookup with source code — one call instead of N find_symbol calls
code(operation="lookup_symbols", symbols="AuthService,verify_token,login", include_source=True)

# AST structural search (ast-grep patterns)
code(operation="pattern_search", pattern="def $F(self, $$$):", language="python")

# Structural rewrite — always dry_run=True first
code(operation="pattern_rewrite",
     pattern="logger.info($MSG)", replacement="logger.debug($MSG)",
     language="python", file_path="src/server.py", dry_run=True)
# Then apply:
code(operation="pattern_rewrite", ..., dry_run=False)

# Explore a directory
code(operation="search_codebase_map", path="src/auth")
```

### Analysis

| Tool | When to use | Key params |
|---|---|---|
| `analyze(path)` | Code smells, complexity, quality score. Scope to a file or directory. | `path` (optional, defaults to whole codebase) |
| `overview()` | Project structure: file count, languages, top dirs, symbol counts. | — |
| `architecture()` | Layers, entry points, hub symbols (most-connected classes). | — |

### Git

| Tool | When to use | Key params |
|---|---|---|
| `commit_search(query)` | Semantic search over git commit history. Finds commits by meaning. | `query`, `author`, `branch`, `limit` (default 10) |
| `commit_history(limit)` | Browse recent commits chronologically. | `limit` (default 50, max 500), `since` (e.g. "2 weeks ago") |

### Cross-Repo

| Tool | When to use | Key params |
|---|---|---|
| `repo_add(path)` | Register another repo for unified search across all repos. | `path`, `name` (optional short name), `index_now` (default true) |
| `repo_remove(path)` | Unregister a repo. | `path` or `name` |
| `repo_status()` | View all registered repos, indexing state, branch info. | — |

### Memory & Knowledge

| Tool | When to use | Key params |
|---|---|---|
| `remember(content)` | Store a note or decision that persists across sessions. | `content`, `memory_type` (note/decision/preference/doc), `tags`, `ttl` (permanent/month/week/day/session) |
| `recall(query)` | Search memories by meaning. | `query`, `limit`, `memory_type`, `tags` |
| `forget(...)` | Delete memories by ID, tags, or type. | `memory_id`, `tags`, `memory_type` |
| `knowledge(command, ...)` | Index and search your own docs, notes, or files. | `command` (add/search/show/remove/update), `name`, `value`, `query` |

### Session

| Tool | When to use | Key params |
|---|---|---|
| `session_snapshot()` | Compressed session state for context recovery. **Always call first after context compaction.** | — |
| `introspect(query)` | Look up Contextia's own tool docs and settings. | `query` or `doc_path` |

---

## Tool Decision Table

| Task | Use | Never use |
|---|---|---|
| Find code by meaning | `search` | grep, readFile |
| Find a symbol definition | `find_symbol` | grep for function name |
| Who calls this? | `find_callers` | manual trace |
| What does this call? | `find_callees` | reading the body |
| Understand a symbol fully | `explain` | readFile |
| What breaks if I change X? | `impact` | manual audit |
| Project structure | `overview` | ls, glob |
| Architecture layers | `architecture` | reading multiple files |
| Code smells, complexity | `analyze` | manual review |
| When was X changed? | `commit_search` | git log |
| Browse recent commits | `commit_history` | git log |
| AST structural search | `code(pattern_search)` | grep |
| List symbols in a file | `code(get_document_symbols)` | readFile |
| Batch symbol lookup | `code(lookup_symbols)` | multiple find_symbol calls |
| Large result set | `retrieve(sandbox_ref)` | ignoring sandbox_ref |
| Store context for later | `remember` | comments |
| Recall stored context | `recall` | re-reading files |
| Index docs/notes | `knowledge` | storing in memory |
| Recover after compaction | `session_snapshot` | starting over |
| Look up Contextia docs | `introspect` | reading README |

---

## Reading Tool Output

Tool responses have two parts:
- `content[0].text` — human-readable formatted text (what you read and present to users)
- `structuredContent` — machine-readable dict (use for programmatic access)

**Search output:**
```
query: authentication flow
confidence: high  total: 3  tokens: 264

  requireAuth  (convex/lib/authorization.ts:103)
  type: typescript   score: 0.896  match: bm25
  ---
  const identity = await getIdentity(ctx);
  ...
```

**When search returns `sandbox_ref`:** the full result set was too large to inline. The response
includes a preview of the top results plus `sandbox_ref: sx_abc12345`. Call `retrieve("sx_abc12345")`
to get the full set. You can keep working from the preview without retrieving — `full_total` tells
you how many results exist.

**find_callers / find_callees output:**
```
symbol: authenticate
total: 4
callers: login (auth/service.py:45), middleware (auth/middleware.py:12), ...
```

**status() key fields:**
- `indexed: true` → ready to use
- `vector_chunks: N` → N code chunks indexed
- `commits_indexed: N` → git history available for commit_search
- `cache.hit_rate: 0.28` → 28% of searches served from cache
- `branch: main` → current branch

---

## Workflows

### 1. Understand an unfamiliar codebase

```
overview()                          → file count, languages, top dirs
architecture()                      → layers, entry points, hub symbols
search("authentication flow")       → find the specific area
explain("AuthService")              → understand the main class
find_callers("login")               → see all call sites
```

### 2. Safe refactoring — ALWAYS this order

```
impact("SymbolToChange")            → ALL transitive callers first
explain("SymbolToChange")           → understand current implementation
find_callers("SymbolToChange")      → full caller list if needed
# Make the change
search("SymbolToChange", mode="bm25")  → verify all references updated
```

**Never suggest a rename/delete/signature change without running `impact` first.**

### 3. Bug investigation

```
search("error message or behavior")    → find the relevant code
find_symbol("ErrorClass")              → locate the definition
find_callers("ErrorClass")             → see where it's raised
commit_search("recent changes to X")   → check git history
explain("TokenBucketRateLimiter")      → full context
```

### 4. AST rewrite

```
code(operation="pattern_search", pattern="...", language="python")  → find matches first
code(operation="pattern_rewrite", ..., dry_run=True)                → preview changes
code(operation="pattern_rewrite", ..., dry_run=False)               → apply
```

### 5. Context recovery after compaction

```
session_snapshot()    → ALWAYS first — restores awareness of what was done
recall("topic")       → retrieve stored decisions and context
```

### 6. Large result sets

```
search("broad query")
# If response has sandbox_ref:
retrieve("sx_abc12345")              → full result set
retrieve("sx_abc12345", query="auth")  → filtered subset
```

---

## Search Best Practices

```python
# Default: hybrid (semantic + keyword + graph) — best quality
search("how does authentication work")

# BM25 for exact names/identifiers
search("TokenBudget", mode="bm25")

# Filter by language or type
search("database connection pool", language="python", symbol_type="class")

# Budget-aware: trim results to fit token budget
search("auth flow", context_budget=500)
```

**Confidence interpretation:**
- `high` → top result is strongly relevant, proceed
- `medium` → relevant but verify with `explain`
- `low` → query too broad; try `find_symbol` with exact name or narrow the query

---

## Anti-Patterns

| Anti-pattern | Why | Instead |
|---|---|---|
| `readFile` before `search` or `find_symbol` | Wastes 50–100x tokens | `search` or `find_symbol` first |
| `grep` for a symbol name | Misses semantic matches | `find_symbol(exact=False)` |
| `index()` before every tool call | Index persists; re-indexing wastes time | Index once; `status()` to check |
| `search` immediately after `index` without `status()` | Index runs in background | Poll `status()` until `indexed: true` |
| Ignoring `sandbox_ref` in search results | Misses most of the results | Call `retrieve(sandbox_ref)` |
| `explain` on utility functions called 200+ times | Huge output | `search` or `find_callers` instead |
| `overview`/`architecture` to find specific code | Wrong tool | `search` for specific questions |
| Rename/delete without `impact` | Breaks callers you didn't know about | Always run `impact` first |
| `limit=100` on search | Relevance threshold already filters noise | Default 10 is enough |
| Re-doing work after compaction | Wastes tokens | `session_snapshot()` first |
| `find_symbol(exact=False)` for broad terms like "auth" | Returns 200+ results | `search` for broad concepts |
| Multiple `find_symbol` calls for related symbols | Slow | `code(lookup_symbols, symbols="A,B,C")` |

---

## Token Efficiency

**Search first, read only if needed:**
```
search("process_payment") → signature + call sites in ~265 tokens
→ only readFile if you need the full implementation body
```

**Progressive disclosure:**
```
explain("PaymentProcessor") → definition + top callers + callees in one call
→ find_callers("PaymentProcessor") if you need ALL callers
→ readFile only if you need the full body
```

**Batch over serial:**
```
code(lookup_symbols, symbols="A,B,C", include_source=True)  → one call
vs. find_symbol("A") + find_symbol("B") + find_symbol("C")  → three calls
```

**Budget-aware for large sessions:**
```
search("auth flow", context_budget=300)  → trims to fit 300 tokens
```

---

## Error Handling

**"No codebase indexed. Run 'index' first."**
→ `index("/path/to/project")`, then poll `status()` until `indexed: true`

**"Symbol 'X' not found."**
→ `find_symbol("X", exact=False)` for fuzzy matching
→ `search("X", mode="bm25")` for keyword search

**`commit_search` returns 0 results**
→ Check `status()` for `commits_indexed > 0`; if 0, re-run `index()` to trigger commit indexing

**`search` confidence=low**
→ Narrow the query or use `find_symbol` with exact name

**`sandbox_ref` in search response**
→ Call `retrieve(sandbox_ref)` to get the full result set
