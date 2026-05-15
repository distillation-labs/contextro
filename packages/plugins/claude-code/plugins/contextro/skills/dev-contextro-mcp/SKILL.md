---
name: dev-contextro-mcp
description: >
  Use Contextro for codebase discovery, semantic/code-graph search, safe refactors,
  AST search and rewrite, git history, memory recovery, and cross-repo search.
  Trigger when the user asks to index or search a codebase, find a symbol or usage,
  explain how code works, trace callers/callees, assess what breaks before a change,
  inspect recent commits, recover after compaction, search across repos, or retrieve
  archived session context. Do not use for general programming questions, writing new code
  from scratch, or reading a single known file when direct file inspection is cheaper.
when_to_use: >
  Prefer Contextro before file-by-file reads for unfamiliar codebases and multi-file
  questions. Especially useful for: who calls, what calls, what breaks, find usages,
  explain this class, git history, pattern search, add repo, remember or recall,
  compact or archive, retrieve archived context.
metadata:
  version: "0.1.0"
  mcp-server: contextro
  category: mcp-enhancement
  tags: [contextro, mcp, code-search, code-graph, ast, git, memory, cross-repo]
license: Proprietary
---

# Contextro MCP

Use Contextro as the default discovery layer for unfamiliar or multi-file code work.
It is best for semantic search, symbol lookup, call graphs, impact analysis, commit
history, memory recovery, cross-repo search, and AST-aware search or rewrite.

Pure Rust binary. No Python. No interpreter.

## Use It For

- Finding code by concept, symbol, caller, callee, or exact identifier.
- Understanding a symbol or project before editing.
- Checking refactor blast radius before rename, delete, or signature changes.
- Investigating regressions with code search plus commit history.
- Recovering after compaction or searching archived session context.
- Searching across multiple repos or retrieving archived session context.

## Do Not Use It For

- General programming questions.
- Writing new code from scratch when no repository discovery is needed.
- Reading one known config file or one known small file directly.
- Any task where a direct single-file read is clearly cheaper than indexing or search.

## First Use In A Repo

```text
1. index("/absolute/path/to/project")
2. If the response returns `status: "done"`, start using search/find_symbol/explain/impact
3. Use `status()` only when readiness is uncertain or you are recovering session state
```

Index persists. Do not re-index before every call. A successful `index()` response is a sufficient readiness signal.

## Routing

| Task | Use | Notes |
|---|---|---|
| Find an exact symbol definition | `find_symbol(name="ExactName")` | `name` param; add `exact=false` for fuzzy |
| Find code by concept | `search(query="authentication middleware")` | Default discovery tool |
| Find exact identifier/string references | `search(query="CTX_STORAGE_DIR", mode="bm25")` | Prefer BM25 for exact names |
| Find callers | `find_callers(symbol_name="Symbol")` | Returns `{callers: [...]}` |
| Find callees | `find_callees(symbol_name="Symbol")` | Returns `{callees: [...]}` |
| Understand one symbol | `explain(symbol_name="Symbol")` | Start here before file reads |
| Map a subsystem or architecture slice | `code(operation="search_codebase_map", query="...")` | Strong for architecture or subsystem mapping; prefer `find_symbol` + `focus`/`explain` for narrow questions |
| Orient in a new codebase | `overview()` then `architecture()` | High-signal orientation path |
| Check refactor impact | `impact(symbol_name="Symbol")` | Mandatory before rename/delete/signature changes |
| Batch lookup several symbols | `code(operation="lookup_symbols", symbols="A,B,C")` | Avoid serial `find_symbol` calls |
| List symbols in a file | `code(operation="get_document_symbols", file_path="...")` | Returns columnar `{ file, columns, symbols, total }`; use `include_signature=true` only when signatures matter |
| List symbols in a directory | `code(operation="list_symbols", path="...")` | Directory mode returns object rows with `callers` and `callees` |
| Structural search | `code(operation="pattern_search", pattern="fn $F($$$)", language="rust")` | Use for AST-shaped queries |
| Structural rewrite | `code(operation="pattern_rewrite", ..., dry_run=true)` | Preview first, then apply |
| Plan an edit | `code(operation="edit_plan", goal="...", symbol_name="...")` | Heuristic planning aid: affected symbols/files, risks, next steps |
| Search commit history | `commit_search(query="...")` or `commit_history(limit=N)` | `commit_search` works best with descriptive commit subjects; use `commit_history` when they are terse |
| Add/search another repo | `repo_add(path="...", name="...")`, `repo_status()`, `search(...)` | Use for cross-repo flows |
| Store a durable decision | `remember(content="...", memory_type="decision")` | Persistent memory |
| Archive pre-compaction context | `compact(content="...")` | Archive path; returns `ref_id` |
| Recover archived context | `retrieve(ref_id="arc_...")` | Retrieves `{ ref_id, content }` |
| Recover durable memory | `recall(query="...")` | Memory path for `remember()` content |

## Parameter Reference

Key parameter names that differ from intuition:

| Tool | Param | Type |
|---|---|---|
| `find_symbol` | `symbol_name` | string (preferred); `name` / `symbol` aliases also work |
| `find_callers` | `symbol_name` | string (required) |
| `find_callees` | `symbol_name` | string (required) |
| `explain` | `symbol_name` | string (required) |
| `impact` | `symbol_name` | string (required); optional `max_depth` int |
| `retrieve` | `ref_id` | string (required, e.g. `"arc_abc123"`) |
| `forget` | `memory_id` or `tags` or `memory_type` | at least one required |

## Response Format

Current search responses use full keys:

- Top-level search response includes `query`, `confidence`, `results`, `total`, and usually `limit` plus `truncated`.
- Each search result uses `name`, `file`, `line`, `type`, and `score`.
- Symbol lookup responses use `{ symbols: [...], total: N }`.
- `get_document_symbols(path)` and `list_symbols(path=<file>)` return `{ file, columns, symbols, total }` where each row in `symbols` is positional against `columns`.
- File-symbol `columns` always start with `name`, `type`, `line`; `end_line` appears only when needed and `signature` appears only when `include_signature=true`.
- `list_symbols(path=<dir>)` is a different contract: `{ path, symbols: [{ name, type, file, line, callers, callees }], total }`.
- `retrieve(ref_id="...")` returns `{ ref_id, content }`.

## Mandatory Workflows

### Safe Refactor

```text
1. impact(symbol_name="Symbol")
2. explain(symbol_name="Symbol")
3. find_callers(symbol_name="Symbol") if impact is broad
4. Make the code change
5. search(query="OldName", mode="bm25") to verify cleanup
```

Never recommend rename, delete, or signature changes without `impact()` first.

### New Codebase Orientation

```text
1. overview()
2. architecture()
3. explain(symbol_name="hub-symbol")
```

This is the default orientation path. Do not start with broad file reads.

### Bug Investigation

```text
1. search(query="error message or symptom")
2. explain(symbol_name="relevant symbol") or find_callers/find_callees
3. Prefer `commit_search(query="recent changes related to symptom")` when commit subjects are descriptive; otherwise use `commit_history(limit=N)`
```

### AST Rewrite

```text
1. code(operation="pattern_search", pattern="...", language="...")
2. code(operation="pattern_rewrite", ..., dry_run=true)
3. Review preview
4. code(operation="pattern_rewrite", ..., dry_run=false)
```

### Recovery After Compaction

```text
1. retrieve(ref_id="arc_...") if you have the archive ref from `compact()`
2. Use `recall(query="topic")` only for durable memory stored via `remember()`
3. Use `session_snapshot()` or `status()` only if you need session re-entry help
```

## Output Rules

- Read `content[0].text` first. Treat it as the primary output.
- Use `structuredContent` only as a supplement.
- If `confidence: low` is present, narrow the query or switch to `find_symbol`.
- Keep default search limits unless the user explicitly wants exhaustive output.
- For narrower responses, lower `limit`; for server-side response budgeting, use `max_tokens` when that wrapper is available.

## Anti-Patterns

- Do not call `status()` after every successful `index()`; `index()` returning `status: "done"` is already enough.
- Do not re-index the same repo repeatedly.
- Do not use three separate `find_symbol` calls when `lookup_symbols` fits.
- Do not use `remember()` for pre-compaction archival; use `compact()`.
- Do not claim `recall(memory_type="archive")` retrieves compacted archives; use `retrieve(ref_id)` for archives and `recall()` for memories.
- Do not use `overview()` to find one symbol; use `search()` or `find_symbol()`.
- Do not use `search_codebase_map` for narrow single-symbol questions when `find_symbol()` plus `focus()` or `explain()` is more direct.
- Do not replace exact-history questions with shell `git log` when `commit_search()` or `commit_history()` is available.
- Do not pass `name=` to `find_callers/find_callees/explain/impact`; use `symbol_name=`.

## Escalation Rule

Prefer Contextro first for discovery. Once it has narrowed the scope to a specific file
or symbol, direct file reads are acceptable if the full implementation body is needed.

## Benchmarks

Current study-backed evidence to cite safely:

| Study | Contextro success | Baseline success | Contextro tokens | Baseline tokens | Reduction | Tool calls/task | Files read |
|---|---|---|---|---|---|---|---|
| Contextro repo, 100 tasks | 100% | 99% | 9,905 | 109,067 | 90.9% | 1.0 | 0 |
| Contextro repo, 200 tasks | 100% | 99% | 23,447 | 222,646 | 89.5% | 1.0 | 0 |
| Production TypeScript monorepo, 1,000 tasks | 100% | 99.5% | 93,819 | 941,748 | 90.0% | 1.0 | 0 |

Useful category notes from the 200-task Contextro repo study:

- `batch_lookup`: 94.4% token reduction
- `document_symbols`: 83.1% token reduction
- `exact_search`: 87.2% token reduction
- `symbol_discovery`: 94.9% token reduction

Use these study numbers instead of older per-tool token estimates or compact-key claims.

## References

- Full routing guide: `references/tool-decision-tree.md`
- Token and benchmark data: `references/benchmark-results.md`
- Eval rubric: `references/eval-rubric.md`
