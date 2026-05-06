---
name: dev-contextia-mcp
description: >
  Code intelligence for coding agents via Contextia MCP — semantic search, call graph traversal,
  impact analysis, AST search/rewrite, git history, persistent memory, and session recovery.
  Use when the user asks to find code, understand a function, trace callers, check what breaks
  before a refactor, search commit history, index a project, store context, or recover session
  state. Trigger phrases: "index this project", "search the codebase", "who calls X", "what does
  X call", "what breaks if I change X", "explain this symbol", "find where X is used", "search
  git history", "what changed recently", "remember this", "recall context", "pattern search",
  "AST search", "find all usages". Always prefer Contextia tools over grep, readFile, and glob —
  they are faster, more accurate, and use up to 9.8x fewer tokens.
metadata:
  version: "2.0.0"
  mcp-server: contextia
  tags: [contextia, mcp, code-search, code-graph, indexing, git, memory, session, ast]
license: MIT
---

# Contextia MCP

Contextia is a fully local code intelligence MCP server. It gives coding agents semantic search, a live call graph, AST-based structural search, git history search, persistent memory, and session recovery — all in one process, no cloud, no API keys, <350MB RAM.

**Core rule: Always prefer Contextia tools over reading files, grepping, or globbing.** A single `search` call returns the exact chunk in <2ms. Reading files wastes 50–100x more tokens for the same information.

---

## Critical: Index First

Before using any tool, ensure the codebase is indexed:

```
1. index("/absolute/path/to/project")   → returns immediately: {"status": "indexing"}
2. status()                              → poll every 3s until indexed: true
3. Now use search, find_symbol, etc.
```

**Never call search/explain/find_callers immediately after index without checking status() first.** Background indexing takes 10-30s for large codebases.

**After server restart:** If `CTX_AUTO_WARM_START=true` (default in Docker), the index restores automatically — skip `index()` and call `status()` to confirm `indexed: true`.

---

## Tool Decision Table

| Task | Tool | NOT |
|---|---|---|
| Find code by meaning | `search` | grep, readFile |
| Find a symbol definition | `find_symbol` | grep for function name |
| Who calls this function? | `find_callers` | manual trace |
| What does this function call? | `find_callees` | reading the body |
| Understand a symbol fully | `explain` | readFile |
| What breaks if I change X? | `impact` | manual audit |
| Project structure | `overview` | ls, glob |
| Architecture layers | `architecture` | reading multiple files |
| Code smells, complexity | `analyze` | manual review |
| When was X changed? | `commit_search` | git log |
| Browse recent commits | `commit_history` | git log |
| AST structural search | `code` (pattern_search) | grep |
| List symbols in a file | `code` (get_document_symbols) | readFile |
| Batch symbol lookup | `code` (lookup_symbols) | multiple find_symbol calls |
| Store context for later | `remember` | comments |
| Recall stored context | `recall` | re-reading files |
| Index docs/notes | `knowledge` | storing in memory |
| Recover after compaction | `session_snapshot` | starting over |
| Look up Contextia docs | `introspect` | reading README |

---

## Workflows

### 1. Understand an unfamiliar codebase

```
overview()                          → language breakdown, top dirs, symbol counts
architecture()                      → layers, entry points, hub symbols
search("authentication flow")       → find the specific area
explain("AuthService")              → understand the main class
find_callers("login")               → see all call sites
```

### 2. Safe refactoring (ALWAYS follow this order)

```
impact("SymbolToChange")            → see ALL transitive callers first
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

### 4. Context recovery after compaction

```
session_snapshot()    → restore awareness of what was done this session
recall("topic")       → retrieve stored decisions and context
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

# Results include: name, file, line, code snippet, match type, confidence
# confidence=high means top result is strongly relevant
# match=semantic+graph means found by both semantic and graph engines
```

**Search result interpretation:**
- `confidence: high` → top result is strongly relevant, proceed with it
- `confidence: medium` → results are relevant but verify with `explain`
- `confidence: low` → query may be too broad; try `find_symbol` with exact name

---

## Code Tool (AST Operations)

```python
# Find all symbols in a file
code(operation="get_document_symbols", file_path="src/auth/service.py")

# Fuzzy symbol search
code(operation="search_symbols", symbol_name="auth", language="python")

# Batch lookup with source code
code(operation="lookup_symbols", symbols="AuthService,verify_token,login", include_source=True)

# AST structural search (ast-grep patterns)
code(operation="pattern_search", pattern="def $FUNC(self, $$$ARGS):", language="python")

# AST rewrite with dry_run preview
code(operation="pattern_rewrite",
     pattern="logger.info($MSG)",
     replacement="logger.debug($MSG)",
     language="python",
     file_path="src/server.py",
     dry_run=True)   # Set dry_run=False to apply
```

---

## Memory and Session

```python
# Store decisions that should survive session compaction
remember("We use JWT with 24h expiry, refresh tokens in Redis", memory_type="decision")
remember("Auth module uses bcrypt cost=12", memory_type="note", tags="auth,security")

# Recall by meaning
recall("JWT token expiry")
recall("authentication approach")

# Index external docs for search
knowledge(command="add", name="API docs", value="/path/to/docs/")
knowledge(command="search", query="rate limiting headers")

# After context compaction — ALWAYS call this first
session_snapshot()
```

---

## Anti-Patterns (Never Do These)

| Anti-pattern | Why | Instead |
|---|---|---|
| `readFile` before `search` or `find_symbol` | Wastes 50-100x tokens | Use `search` or `find_symbol` first |
| `grep` for a symbol name | Misses semantic matches | Use `find_symbol(exact=False)` |
| `index()` before every tool call | Index persists; re-indexing wastes time | Index once; use `status()` to check |
| `search` immediately after `index` without `status()` | Index runs in background | Poll `status()` until `indexed: true` |
| `explain` on utility functions called 200+ times | Huge output | Use `search` or `find_callers` instead |
| `overview`/`architecture` to find specific code | Wrong tool | Use `search` for specific questions |
| Rename/delete without `impact` | Breaks callers you didn't know about | Always run `impact` first |
| `limit=100` on search | Relevance threshold already filters noise | Default 10 is usually enough |
| Re-doing work after compaction | Wastes tokens | Call `session_snapshot()` first |
| `find_symbol(exact=False)` for broad terms like "auth" | Returns 200+ results | Use `search` for broad concepts |

---

## Token Efficiency Patterns

**Pattern 1: Search first, read only if needed**
```
search("process_payment") → get signature + call sites
→ only readFile if you need the full implementation body
```

**Pattern 2: Narrow queries beat broad dumps**
```
search("auth middleware") → 3 precise results, ~265 tokens
overview() → compact summary, ~18 tokens for top-level dirs
→ search is almost always more useful than overview for specific questions
```

**Pattern 3: Progressive disclosure with explain**
```
explain("PaymentProcessor") → compact: definition + top 15 callers + total count
→ if you need ALL callers: find_callers("PaymentProcessor")
→ if you need the full body: readFile with the line number from explain
```

**Pattern 4: Use context_budget for large sessions**
```
search("auth flow", context_budget=300)  → trims results to fit 300 tokens
```

---

## Status and Health

```python
status()   # Shows: indexed, vector_chunks, graph stats, branch, commits_indexed,
           # cache hit rate, memory usage
health()   # Readiness check — use before any tool calls in automated pipelines
```

**status() output key fields:**
- `indexed: true` → ready to use
- `commits_indexed: N` → git history available for commit_search
- `cache.hit_rate: 0.28` → 28% of searches served from cache (fast)
- `branch: main` → current branch being indexed

---

## Error Handling

**"No codebase indexed. Run 'index' first."**
→ Call `index("/path/to/project")`, then poll `status()` until `indexed: true`

**"Symbol 'X' not found."**
→ Try `find_symbol("X", exact=False)` for fuzzy matching
→ Or `search("X", mode="bm25")` for keyword search

**commit_search returns 0 results**
→ Check `status()` for `commits_indexed > 0`
→ If 0, re-run `index()` to trigger commit indexing

**search confidence=low**
→ Query may be too broad; try `find_symbol` with exact name
→ Or narrow the query: "auth middleware" instead of "auth"

---

## References

- `references/tool-decision-tree.md` — full routing table with examples
- `references/benchmark-results.md` — token efficiency benchmarks
