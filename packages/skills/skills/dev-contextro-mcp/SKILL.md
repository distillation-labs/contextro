---
name: dev-contextro-mcp
description: >
  Use Contextro for codebase discovery, semantic/code-graph search, safe refactors,
  AST search and rewrite, git history, memory recovery, and cross-repo search.
  Trigger when the user asks to index or search a codebase, find a symbol or usage,
  explain how code works, trace callers/callees, assess what breaks before a change,
  inspect recent commits, recover after compaction, search across repos, or retrieve
  sandboxed results. Do not use for general programming questions, writing new code
  from scratch, or reading a single known file when direct file inspection is cheaper.
when_to_use: >
  Prefer Contextro before file-by-file reads for unfamiliar codebases and multi-file
  questions. Especially useful for: who calls, what calls, what breaks, find usages,
  explain this class, git history, pattern search, add repo, remember or recall,
  compact or archive, retrieve sandbox results.
metadata:
  version: "0.0.5"
  mcp-server: contextro
  category: mcp-enhancement
  tags: [contextro, mcp, code-search, code-graph, ast, git, memory, cross-repo]
license: Proprietary
---

# Contextro MCP

Use Contextro as the default discovery layer for unfamiliar or multi-file code work.
It is best for semantic search, symbol lookup, call graphs, impact analysis, commit
history, memory recovery, cross-repo search, and AST-aware search or rewrite.

## Use It For

- Finding code by concept, symbol, caller, callee, or exact identifier.
- Understanding a symbol or project before editing.
- Checking refactor blast radius before rename, delete, or signature changes.
- Investigating regressions with code search plus commit history.
- Recovering after compaction or searching archived session context.
- Searching across multiple repos or retrieving sandboxed large results.

## Do Not Use It For

- General programming questions.
- Writing new code from scratch when no repository discovery is needed.
- Reading one known config file or one known small file directly.
- Any task where a direct single-file read is clearly cheaper than indexing or search.

## First Use In A Repo

```text
1. index("/absolute/path/to/project")
2. status()
3. Wait for codebase_path to appear before search/find_symbol/explain/impact
```

Index persists. Do not re-index before every call. Use `status()` to check readiness.

## Routing

| Task | Use | Notes |
|---|---|---|
| Find an exact symbol definition | `find_symbol("ExactName")` | Best when the exact name is known |
| Find code by concept | `search("authentication middleware")` | Default discovery tool |
| Find exact identifier/string references | `search("CTX_STORAGE_DIR", mode="bm25")` | Prefer BM25 for exact names |
| Find callers | `find_callers("Symbol")` | Returns `{callers: [...]}` |
| Find callees | `find_callees("Symbol")` | Returns `{callees: [...]}` |
| Understand one symbol | `explain("Symbol")` | Start here before file reads |
| Orient in a new codebase | `overview()` then `architecture()` | High-signal orientation path |
| Check refactor impact | `impact("Symbol")` | Mandatory before rename/delete/signature changes |
| Batch lookup several symbols | `code(operation="lookup_symbols", symbols="A,B,C")` | Avoid serial `find_symbol` calls |
| List symbols in a file | `code(operation="get_document_symbols", file_path="...")` | Better than reading for structure |
| Structural search | `code(operation="pattern_search", ...)` | Use for AST-shaped queries |
| Structural rewrite | `code(operation="pattern_rewrite", dry_run=True)` | Preview first, then apply |
| Search commit history | `commit_search("query")` or `commit_history(...)` | Prefer over shell `git log` |
| Add/search another repo | `repo_add(...)`, `repo_status()`, `search(...)` | Use for cross-repo flows |
| Store a durable decision | `remember(content, memory_type="decision")` | Persistent memory |
| Archive pre-compaction context | `compact(content)` | Not `remember()` |
| Recover after compaction | `session_snapshot()` then `recall(...)` | Search archive with `memory_type="archive"` |
| Expand sandboxed large output | `retrieve("sx_...")` | Use when `sandbox_ref` is present |

## Response Format

Search results use compact keys to minimise token usage:

| Key | Meaning |
|---|---|
| `n` | symbol name |
| `f` | file path (relative) |
| `l` | start line |
| `c` | code snippet (top result only) |
| `t` | type (omitted when `function`) |
| `lc` | line count |
| `doc` | docstring (first sentence) |

`confidence` is omitted when high (the default). `sandboxed` is omitted — presence
of `sandbox_ref` implies sandboxing. `lang` is omitted for Python (the default).

## Mandatory Workflows

### Safe Refactor

```text
1. impact("Symbol")
2. explain("Symbol")
3. find_callers("Symbol") if impact is broad
4. Make the code change
5. search("OldName", mode="bm25") to verify cleanup
```

Never recommend rename, delete, or signature changes without `impact()` first.

### New Codebase Orientation

```text
1. overview()
2. architecture()
3. explain("hub-symbol")
```

This is the default orientation path. Do not start with broad file reads.

### Bug Investigation

```text
1. search("error message or symptom")
2. explain("relevant symbol") or find_callers/find_callees
3. commit_search("recent changes related to symptom")
```

### AST Rewrite

```text
1. code(operation="pattern_search", ...)
2. code(operation="pattern_rewrite", dry_run=True, ...)
3. Review preview
4. code(operation="pattern_rewrite", dry_run=False, ...)
```

### Recovery After Compaction

```text
1. session_snapshot()
2. recall("topic", memory_type="archive")
3. recall("topic")
```

## Output Rules

- Read `content[0].text` first. Treat it as the primary output.
- Use `structuredContent` only as a supplement.
- If `confidence: low` is present, narrow the query or switch to `find_symbol`.
- If the response includes `sandbox_ref`, call `retrieve()` before claiming you have the full result set.
- Keep default search limits unless the user explicitly wants exhaustive output.
- If the user mentions a tight context budget, pass `context_budget` to `search()`.

## Anti-Patterns

- Do not call `search()` immediately after `index()`; wait for `status()`.
- Do not re-index the same repo repeatedly.
- Do not use three separate `find_symbol` calls when `lookup_symbols` fits.
- Do not ignore `sandbox_ref` in responses.
- Do not use `remember()` for pre-compaction archival; use `compact()`.
- Do not use `overview()` to find one symbol; use `search()` or `find_symbol()`.
- Do not replace exact-history questions with shell `git log` when `commit_search()` or `commit_history()` is available.

## Escalation Rule

Prefer Contextro first for discovery. Once it has narrowed the scope to a specific file
or symbol, direct file reads are acceptable if the full implementation body is needed.

## Benchmarks

Token efficiency (16-tool workflow):

| Tool | Tokens |
|---|---|
| `search` | 116 |
| `explain` | 43 |
| `find_symbol` | 36 |
| `find_callers` | 6 |
| `status` | 20 |
| Total (16 calls) | 1,043 |

Retrieval quality (20 queries, src codebase):

| Metric | Value |
|---|---|
| Hybrid MRR | 1.000 |
| Hybrid recall@1 | 1.000 |
| Avg tokens/query | 152 |
| Avg latency | 4.4 ms |

Indexing (potion-code-16m, 76 files / 1,620 chunks):

| Metric | Value |
|---|---|
| Full index | 0.45 s |
| Incremental (no changes) | 22 ms |

## References

- Full routing guide: `references/tool-decision-tree.md`
- Token and benchmark data: `references/benchmark-results.md`
- Eval rubric: `references/eval-rubric.md`
