---
name: dev-contextia-mcp
description: >
  Code intelligence superpower for coding agents. 26 tools that replace grep, readFile, glob,
  and git log with semantic search (<2ms), call graph traversal, impact analysis, AST rewrite,
  persistent memory, and progressive disclosure. 9.8x fewer tokens than file reading.
  Trigger: "index", "search", "who calls", "what calls", "what breaks", "explain", "find symbol",
  "find usages", "git history", "remember", "recall", "pattern search", "AST", "add repo",
  "retrieve", "compact", "archive". ALWAYS prefer Contextia over grep/readFile/glob.
metadata:
  version: "3.1.0"
  mcp-server: contextia
  tags: [contextia, mcp, code-search, code-graph, indexing, git, memory, session, ast, cross-repo, progressive-disclosure]
license: MIT
---

# Contextia MCP — Agent Superpower

**One rule: NEVER read files or grep when Contextia is available.**

| Without Contextia | With Contextia | Savings |
|---|---|---|
| Read 5 files to find one function (~5000 tokens) | `search("auth flow")` (~265 tokens) | **19x** |
| grep + read 3 files to trace callers (~3000 tokens) | `find_callers("login")` (~16 tokens) | **187x** |
| Read file + grep to understand a class (~2000 tokens) | `explain("AuthService")` (~230 tokens) | **9x** |
| Manual audit of 10 files before rename (~8000 tokens) | `impact("TokenBudget")` (~300 tokens) | **27x** |

---

## Quick Start (Do Once)

```
index("/absolute/path/to/project")   # returns immediately
status()                              # poll until indexed: true (10-30s)
# Now use any tool
```

Index persists on disk. Never re-index unless you switch projects.

---

## Tool Picker — Use This Decision Tree

```
"I need to find code"
  ├─ Know the exact symbol name? → find_symbol("ExactName")
  ├─ Know the concept but not the name? → search("concept description")
  ├─ Need exact string/identifier match? → search("IDENTIFIER", mode="bm25")
  └─ Need multiple symbols at once? → code(lookup_symbols, symbols="A,B,C")

"I need to understand code"
  ├─ One symbol fully? → explain("Symbol")
  ├─ Who calls it? → find_callers("Symbol")
  ├─ What does it call? → find_callees("Symbol")
  ├─ All symbols in a file? → code(get_document_symbols, file_path="...")
  └─ Project structure? → overview() then architecture()

"I need to change code"
  ├─ Before ANY rename/delete/signature change → impact("Symbol") FIRST
  ├─ Structural find-and-replace? → code(pattern_rewrite, dry_run=True) then dry_run=False
  └─ Verify all references updated? → search("OldName", mode="bm25")

"I need history"
  ├─ What changed related to X? → commit_search("X changes")
  └─ Recent commits? → commit_history(limit=10)

"I need to remember/recover"
  ├─ Store a decision → remember(content, memory_type="decision")
  ├─ Before context compaction → compact(content)
  ├─ After context compaction → session_snapshot() FIRST, then recall()
  └─ Search archived sessions → recall(query, memory_type="archive")

"Response has sandbox_ref"
  └─ Need full results? → retrieve("sx_abc12345")
```

---

## All 26 Tools — Complete Reference

### 🔍 Search & Discovery

| Tool | Cost | When | Example |
|---|---|---|---|
| `search(query)` | ~265 tok | Find code by meaning or keyword | `search("rate limiting", language="python")` |
| `find_symbol(name)` | ~66 tok | Find exact definition | `find_symbol("IndexingPipeline")` |
| `find_callers(symbol)` | ~16 tok | Who calls this? | `find_callers("authenticate")` |
| `find_callees(symbol)` | ~16 tok | What does this call? | `find_callees("process_request")` |
| `explain(symbol)` | ~230 tok | Full context: def + callers + callees + related | `explain("SearchEngine", verbosity="summary")` |
| `impact(symbol)` | ~300 tok | What breaks if I change this? (transitive) | `impact("BaseModel", max_depth=5)` |
| `retrieve(ref_id)` | varies | Fetch sandboxed large result | `retrieve("sx_abc12345", query="auth")` |

**search params:** `mode` (hybrid/vector/bm25), `language`, `symbol_type`, `limit` (default 10), `context_budget`, `rerank` (default true)

### 🧠 Code Intelligence (AST)

| Operation | When | Example |
|---|---|---|
| `get_document_symbols` | List all symbols in a file | `code(operation="get_document_symbols", file_path="src/server.py")` |
| `search_symbols` | Fuzzy symbol search | `code(operation="search_symbols", symbol_name="auth")` |
| `lookup_symbols` | Batch lookup (replaces N find_symbol calls) | `code(operation="lookup_symbols", symbols="A,B,C", include_source=True)` |
| `pattern_search` | AST structural search | `code(operation="pattern_search", pattern="def $F(self, $$$):", language="python")` |
| `pattern_rewrite` | Structural find-replace | `code(operation="pattern_rewrite", pattern="...", replacement="...", dry_run=True)` |
| `search_codebase_map` | Explore a directory | `code(operation="search_codebase_map", path="src/auth")` |

### 📊 Analysis

| Tool | When | Example |
|---|---|---|
| `overview()` | Project structure, languages, file counts | First call on unfamiliar codebase |
| `architecture()` | Layers, entry points, hub symbols | After overview, before diving in |
| `analyze(path)` | Code smells, complexity, quality score | `analyze("src/engines")` |

### 📜 Git History

| Tool | When | Example |
|---|---|---|
| `commit_search(query)` | Find commits by meaning | `commit_search("auth refactoring", author="alice")` |
| `commit_history(limit)` | Browse recent commits | `commit_history(since="1 week ago")` |

### 🗂️ Cross-Repo

| Tool | When | Example |
|---|---|---|
| `repo_add(path)` | Add repo for unified search | `repo_add("/path/to/other-repo")` |
| `repo_remove(path)` | Remove repo | `repo_remove(name="old-backend")` |
| `repo_status()` | View all repos | Check indexing state |

### 💾 Memory & Context

| Tool | When | Example |
|---|---|---|
| `remember(content)` | Persist a note/decision across sessions | `remember("Use RS256 for JWT", memory_type="decision", ttl="permanent")` |
| `recall(query)` | Search memories | `recall("JWT configuration")` |
| `recall(query, memory_type="archive")` | Search compaction archive | `recall("auth investigation", memory_type="archive")` |
| `forget(...)` | Delete memories | `forget(tags="outdated")` |
| `compact(content)` | Archive context BEFORE compaction | `compact("Key findings: ...")` |
| `knowledge(command, ...)` | Index/search your own docs | `knowledge(command="search", query="API rate limits")` |

### ⚙️ Session & Status

| Tool | When | Example |
|---|---|---|
| `index(path)` | Index a codebase (once) | `index("/Users/alice/project")` |
| `status()` | Check if indexed, get stats | Poll after index() |
| `health()` | Readiness check | Automated pipelines |
| `session_snapshot()` | Recover after compaction | ALWAYS first after compaction |
| `introspect(query)` | Look up Contextia's own docs | `introspect("pattern_search syntax")` |

---

## Critical Workflows

### Refactoring (MANDATORY ORDER)

```
1. impact("Symbol")              ← NEVER skip. Shows ALL transitive callers.
2. explain("Symbol")             ← Understand current implementation
3. find_callers("Symbol")        ← Full caller list if impact shows many
4. [Make the change]
5. search("Symbol", mode="bm25") ← Verify all references updated
```

**RULE: Never suggest rename/delete/signature change without impact() first.**

### Codebase Orientation (3 calls, not 15 file reads)

```
1. overview()        ← languages, file count, top directories
2. architecture()    ← layers, entry points, hub symbols
3. explain("Hub")    ← understand the most-connected class
```

### Bug Investigation

```
1. search("error message")       ← find relevant code
2. explain("ErrorSymbol")        ← understand it fully
3. commit_search("recent X")     ← check if recently introduced
```

### Context Compaction Recovery

```
1. session_snapshot()                              ← ALWAYS first
2. recall("what I was working on", memory_type="archive")  ← archived context
3. recall("decisions made")                        ← persistent memories
```

### Before Compaction (save your work)

```
compact("Key findings: X. Decisions: Y. Modified files: Z. Next steps: W.")
```

---

## Progressive Disclosure (Automatic)

Responses >1200 tokens are automatically sandboxed:
- You get a **compact preview** (top results + metadata)
- Plus a `sandbox_ref: sx_abc12345`
- Call `retrieve("sx_abc12345")` ONLY if you need the full output
- Call `retrieve("sx_abc12345", query="filter")` for a filtered subset

**This saves ~44% tokens on large responses.** Most of the time the preview is enough.

---

## Search Modes — Pick the Right One

| Situation | Mode | Example |
|---|---|---|
| Understand a concept | `hybrid` (default) | `search("how does caching work")` |
| Find exact identifier | `bm25` | `search("CTX_STORAGE_DIR", mode="bm25")` |
| Semantic similarity only | `vector` | `search("retry with backoff", mode="vector")` |
| Budget-constrained | any + `context_budget` | `search("auth", context_budget=300)` |

**Confidence signals:**
- `high` → proceed with top result
- `medium` → verify with `explain()`
- `low` → query too broad, narrow it or use `find_symbol`

---

## NEVER Do These

| ❌ Anti-pattern | ✅ Instead | Why |
|---|---|---|
| `readFile("auth.py")` to find a function | `search("auth function")` | 19x fewer tokens |
| `grep "functionName"` | `find_symbol("functionName")` | Semantic + exact match |
| `index()` before every tool call | `status()` to check | Index persists |
| `search()` right after `index()` | Poll `status()` first | Background indexing |
| Ignore `sandbox_ref` in response | `retrieve(sandbox_ref)` | You're missing results |
| Rename without `impact()` | `impact()` first | Breaks unknown callers |
| `find_symbol(exact=False, name="auth")` | `search("authentication")` | Broad terms → search |
| 3× `find_symbol("A")` + `find_symbol("B")` + ... | `code(lookup_symbols, symbols="A,B,C")` | 1 call vs 3 |
| `limit=100` on search | Default 10 | Relevance filter handles noise |
| Re-do work after compaction | `session_snapshot()` first | Recovers state |
| Lose context before compaction | `compact(content)` | Archives for later |
| `remember()` for pre-compaction archive | `compact()` | Different purpose |
| `overview()` to find specific code | `search()` | Wrong tool |

---

## Output Format

All tools return `content[0].text` (human-readable) + optional `structuredContent` (machine-readable).

**Search:**
```
query: auth flow    confidence: high    total: 3    tokens: 264
  requireAuth (src/auth.ts:103)  type: typescript  score: 0.896  match: bm25
  ---
  const identity = await getIdentity(ctx); ...
```

**find_callers/find_callees:**
```
symbol: authenticate    total: 4
callers: login (auth/service.py:45), middleware (auth/middleware.py:12), ...
```

**status():**
```
indexed: true    vector_chunks: 1286    commits_indexed: 150
branch: main    cache.hit_rate: 0.28
```

---

## Error Recovery

| Error | Fix |
|---|---|
| "No codebase indexed" | `index("/path")` → poll `status()` |
| "Symbol not found" | `find_symbol("X", exact=False)` or `search("X", mode="bm25")` |
| `commit_search` returns 0 | Check `status()` → `commits_indexed > 0`; re-index if 0 |
| `confidence: low` | Narrow query or use `find_symbol` with exact name |
| `sandbox_ref` in response | `retrieve(sandbox_ref)` for full results |

---

## Token Budget Reference

| Tool | Typical output | vs readFile equivalent |
|---|---|---|
| `search` | 265 tokens | 5000+ (reading 5 files) |
| `find_callers` | 16 tokens | 3000+ (grep + read) |
| `find_callees` | 16 tokens | 2000+ (read function body) |
| `explain` | 230 tokens | 2000+ (read file + grep callers) |
| `impact` | 300 tokens | 8000+ (manual audit) |
| `find_symbol` | 66 tokens | 500+ (grep + read) |
| `overview` | 200 tokens | 1000+ (ls + glob) |
| `status` | 144 tokens | N/A |

**Total savings per typical session: 65-90% fewer tokens.**
