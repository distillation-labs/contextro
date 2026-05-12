# Contextro MCP — Evaluation Report

**Version:** 0.2.0  
**Date:** 2026-05-12  
**Tested on:** This repository (`distillation-labs/contextro`) — 50 files, 419 symbols, Rust workspace  
**Method:** Every one of the 35 MCP tools called in a single indexed stdio session. Raw JSON responses inspected for correctness.

---

## Overall Scorecard

| Category | Status | Tools |
|---|---|---|
| Indexing & server health | ✅ All working | `index`, `status`, `health` |
| Search — BM25 | ✅ Working | `search(mode=bm25)` |
| Search — vector & hybrid | ❌ Fall through to BM25 | `search(mode=vector)`, `search(mode=hybrid)` |
| Symbol lookup | ✅ Working | `find_symbol` |
| Call graph tools | ❌ All empty | `find_callers`, `find_callees`, `impact`, `explain` |
| Static analysis | ✅ Working | `analyze`, `focus`, `dead_code`, `circular_dependencies`, `audit` |
| Test coverage | 🟡 Runs, inaccurate | `test_coverage_map` |
| Code / AST | 🟡 One op works, one doesn't | `code(pattern_search)` ✅ · `code(list_symbols)` ❌ |
| Memory — core | ✅ Working | `remember`, `recall` |
| Memory — tags & forget | ❌ Broken | `forget(tags=...)` · tags silently dropped on `remember` |
| Knowledge base | ✅ Working | `knowledge(add)`, `knowledge(search)` |
| Git history | ✅ Working | `commit_history`, `commit_search` |
| Session / archive | 🟡 Compact works, retrieve broken | `session_snapshot`, `restore`, `compact` ✅ · `retrieve` ❌ |
| Docs & export | ✅ Working | `docs_bundle`, `sidecar_export`, `skill_prompt` |
| Repo management | ✅ Working | `repo_add`, `repo_remove`, `repo_status` |
| Introspect | ❌ Always empty | `introspect` |
| Architecture / overview | 🟡 Runs, sparse data | `overview`, `architecture` |

**Working: ~27 / 35 tools return meaningful data.**  
The majority of failures trace back to a single root cause — see below.

---

## The One Root Cause Behind Most Failures

**Call graph edges are never built during `index()`.**

```json
// Result of index() on every run:
{"graph_nodes": 419, "graph_relationships": 0, ...}
```

`graph_nodes` is 419 (symbols extracted correctly by tree-sitter), but `graph_relationships` is always 0. No call edges are ever recorded. Every tool that depends on the graph — callers, callees, impact, explain, architecture hub degrees, focus per-symbol edges — returns empty results as a direct consequence.

**Why it's probably happening:** tree-sitter parsing extracts symbol *declarations* (functions, structs, impls) but the pipeline step that resolves *call expressions* and writes edges into `petgraph` is either not running or silently failing. The graph data structure exists in memory (`contextro-core/src/graph.rs`, `contextro-engines/src/graph.rs`) but nothing is populating it during the indexing walk in `contextro-indexing/src/pipeline.rs`.

Fix this one thing and 6+ tools become functional immediately.

---

## Tool-by-Tool Breakdown

### ✅ `index(path)`

Works. Indexes 50 files, extracts 419 symbols in under 0.01s. Warm-start and incremental re-index are fast.

```json
{
  "graph_nodes": 419,
  "graph_relationships": 0,
  "status": "done",
  "time_seconds": 0.01,
  "total_chunks": 419,
  "total_files": 50,
  "total_symbols": 419
}
```

**Issue:** `graph_relationships` is always 0. See root cause above.

---

### ✅ `status()`

Works. Reports indexed path, graph node count, memory count, uptime, and cache hit rate.

---

### ✅ `health()`

Works. Returns `{ "indexed": true, "status": "healthy", "uptime_seconds": 0.0 }`.

---

### 🟡 `search(query, mode)`

**BM25 works well. Vector and hybrid do not.**

All three modes (`hybrid`, `vector`, `bm25`) return identical results with identical scores, all tagged `match=bm25`. The `mode` parameter is accepted without error but has no effect — every query goes through BM25 regardless.

```
mode=hybrid → bm25.rs:220 [match=bm25] score=1.000
mode=vector → bm25.rs:220 [match=bm25] score=1.000  ← same
mode=bm25   → bm25.rs:220 [match=bm25] score=1.000  ← same
```

BM25 quality is good: relevant symbols, high confidence, fast. The improvement from v0.1.0 is real — search was returning 0 results in all modes before and now returns high-confidence hits.

**Why vector isn't working:** LanceDB is likely not being populated with embeddings during `index()`. The potion-code-16m embedding model is supposed to encode each chunk, write vectors to LanceDB, and those vectors get queried during `mode=vector`. If the embedding step is skipped or silently erroring, LanceDB stays empty and the fallback is BM25 every time. Check `contextro-indexing/src/pipeline.rs` for where embeddings are generated and written to the vector store.

---

### ✅ `find_symbol(name, exact)`

Works. Fuzzy match correctly finds `IndexingPipeline` at the right file and line.

```json
{
  "symbols": [
    {"file": "crates/contextro-indexing/src/pipeline.rs", "line": 34,
     "name": "IndexingPipeline", "type": "class", "language": "rust"}
  ],
  "total": 1
}
```

---

### ❌ `find_callers(symbol_name)`

Broken. Always returns `{ "callers": [], "total": 0 }`.

**Why:** Depends entirely on call graph edges. With `graph_relationships: 0`, there is nothing to traverse. Fix the graph builder and this works automatically.

---

### ❌ `find_callees(symbol_name)`

Broken. Always returns `{ "callees": [], "total": 0 }`.

**Why:** Same as `find_callers` — depends on graph edges that don't exist.

---

### ❌ `explain(symbol_name)`

Partially broken. Finds *a* symbol by name but not necessarily the right one — `explain("search")` resolves to a helper constructor in `errors.rs` rather than the main search function, because there's no ranking by relevance among multiple symbols with the same name. Returns `callers: []` and `callees: []` (graph empty). Docstring is always `null`.

```json
{
  "name": "search",
  "file": "crates/contextro-core/src/errors.rs",  ← wrong symbol resolved
  "line": 73,
  "callers": [],   ← empty, graph issue
  "callees": [],   ← empty, graph issue
  "docstring": null
}
```

**Why:** Two issues — symbol disambiguation needs scoring (pick the most-called or most-central symbol when names collide), and callers/callees need the graph.

---

### ❌ `impact(symbol_name, max_depth)`

Broken. Always returns `{ "impacted": [], "total_impacted": 0 }`.

**Why:** Impact analysis walks the call graph outward from a symbol. With 0 edges, there is nothing to traverse at any depth.

---

### ✅ `overview()`

Runs and returns data. Reports codebase path, total symbols (419), and vector chunk count. Sparse — missing language breakdown, top directories, and entry points that are advertised.

```json
{
  "codebase_path": "...",
  "total_symbols": 419,
  "total_relationships": 0,
  "vector_chunks": 419
}
```

**Why it's sparse:** Language and directory aggregation isn't being computed, and the `total_relationships: 0` graph issue means any graph-derived insights are empty.

---

### 🟡 `architecture()`

Runs. Returns hub symbols but all have `degree: 0`.

```json
{
  "hub_symbols": [
    {"name": "fmt", "degree": 0, "file": "...models.rs"},
    {"name": "add", "degree": 0, "file": "...traits.rs"},
    ...
  ]
}
```

**Why:** Hub symbols are ranked by connectivity degree in the call graph. With 0 edges, every symbol has degree 0. Fix the graph builder and this becomes meaningful.

---

### ✅ `dead_code()`

Works. Flags 50 symbols across the codebase with file paths and line numbers. Results look plausible (e.g., `storage_path`, `lancedb_path` in config, various internal helpers).

**Caveat:** Dead code detection without a real call graph is heuristic-based (symbols never referenced in any parsed call expression). May have false positives for symbols used via trait dispatch or macros.

---

### ✅ `circular_dependencies()`

Works. Returns `[]` — correct, no cycles detected in this codebase.

---

### 🟡 `test_coverage_map()`

Runs and returns a list of uncovered files. Reports 0% coverage and 1 test file, which is inaccurate — this repo has integration tests. The coverage mapping is likely only scanning for `#[test]` annotations at the top level rather than recognising test modules or the `tests/` directory.

---

### ✅ `analyze(path)`

Works. Returns large files by symbol count and flags dense modules. Correctly identifies `study.rs` (54 symbols) and `graph.rs` (27 symbols).

```json
{
  "large_files": [
    {"file": "crates/contextro-server/src/study.rs", "symbols": 54},
    {"file": "crates/contextro-core/src/graph.rs",   "symbols": 27}
  ],
  "high_connectivity_symbols": []  ← empty, graph issue
}
```

---

### ✅ `focus(path)`

Works. Returns a compact symbol list for a file — names, types, and line numbers. Good for tight context slices. Per-symbol `callers` and `callees` are 0 (graph issue) but the symbol enumeration itself is correct.

---

### 🟡 `code(operation, ...)`

Mixed.

- `code(operation="pattern_search", pattern="fn search", path="crates/")` — ✅ Works. Finds all `fn search` definitions across the codebase with correct file and line.
- `code(operation="list_symbols", path=...)` — ❌ Returns `{"error": "Unknown code operation: list_symbols"}`. The operation is implied by the tool description but not implemented.

---

### ✅ `remember(content, tags, memory_type)`

Core works — memory is stored, gets an ID, and is retrievable by `recall`. 

**Bug:** The `tags` parameter is accepted without error but silently dropped. Every memory is stored with `"tags": []` regardless of what was passed.

---

### ✅ `recall(query, limit)`

Works. Finds stored memories by semantic similarity. Clean results.

---

### ❌ `forget(tags)` / `forget(memory_id)`

Broken when called with `tags`. Returns:

```json
{"error": "Provide memory_id, tags, or memory_type to forget"}
```

…even when `tags` is provided. The schema advertises the parameter but the handler doesn't read it. Calling with `memory_id` directly works.

---

### ✅ `knowledge(command="add", name, value)` / `knowledge(command="search", query)`

Both work. Add indexes a document snippet, search retrieves it. Clean round-trip.

---

### ✅ `commit_history(limit)`

Works. Returns recent commits with hash, author, message, and Unix timestamp.

---

### ✅ `commit_search(query, limit)`

Works. Semantic search over commit messages returns scored results. Finds relevant commits for natural-language queries.

---

### ✅ `compact(content)`

Works. Archives content and returns a `ref_id`.

```json
{"archived": true, "chars": 62, "ref_id": "arc_03be4610"}
```

---

### ❌ `retrieve(ref_id)`

Broken. Immediately returns `{"error": "Reference 'arc_03be4610' not found or expired."}` for a `ref_id` returned by `compact` in the same session. The archive and retrieval store are not using the same key space, or the archive isn't being persisted before the retrieval attempt.

---

### ✅ `session_snapshot()`

Works. Returns a log of tool calls made in the current session with types and summaries. Useful for giving the agent context about what it has already done.

---

### ✅ `restore()`

Works. Returns the indexed codebase path, graph stats, and an agent hint for re-entry.

---

### ✅ `docs_bundle(output_dir)`

Works. Generates `architecture.md` and `overview.md` into `.contextro-docs/`. Files are written to disk.

---

### ✅ `sidecar_export(path)`

Works. Exports `.graph.*` sidecar files for a given source file.

---

### ❌ `introspect(query)`

Broken. Always returns `{"matching_tools": [], "total": 0}` regardless of query. Should surface relevant tool descriptions to help an agent decide which tool to use — useful for self-discovery. Currently non-functional.

**Why it's probably broken:** The tool index that `introspect` searches is likely empty — the tool descriptions aren't being registered into the searchable index at startup.

---

### ✅ `skill_prompt()`

Works. Returns a well-formatted agent bootstrap block with the key tools and their usage patterns.

---

### ✅ `repo_add(path)` / `repo_remove(path)` / `repo_status()`

All work. Add registers a path, status shows it with git detection, remove unregisters it.

---

## Prioritised Fix List

These are ordered by impact — fixing the top items unblocks the most tools.

### 1. Build call graph edges during `index()` — **blocks 6+ tools**

`graph_relationships` is always 0. The tree-sitter parser extracts symbol declarations but does not extract call expressions and write edges into petgraph. 

**Where to look:** `contextro-indexing/src/pipeline.rs` — the indexing walk needs a pass that resolves call sites and emits `(caller_symbol, callee_symbol)` edges into the graph. Also check `contextro-engines/src/graph.rs` — `add_relationship()` may exist but is never called from the pipeline.

**Tools unblocked:** `find_callers`, `find_callees`, `impact`, `explain` (graph fields), `architecture` (hub degrees), `focus` (per-symbol edges), `dead_code` (accuracy), `analyze` (high-connectivity symbols).

---

### 2. Populate LanceDB with embeddings during `index()` — **makes search truly hybrid**

All three search modes produce identical BM25 results. The vector store is either not being written during indexing or the query path never reaches it.

**Where to look:** `contextro-indexing` — wherever chunks are created, embeddings should be generated (potion-code-16m model) and written to LanceDB. Check if the embedding step is gated behind a flag or silently failing. Also check `contextro-engines` for the vector query path — it may be falling back to BM25 on an empty vector store.

**Tools unblocked:** `search(mode=vector)`, `search(mode=hybrid)`.

---

### 3. Fix `compact` / `retrieve` round-trip — **1 tool**

`retrieve(ref_id)` fails immediately after `compact` returns that same `ref_id` in the same session.

**Where to look:** The archive store write and the lookup key format. Likely a path or namespace mismatch between where `compact` writes and where `retrieve` reads.

---

### 4. Fix `remember` tags — **memory system**

Tags are accepted in the API but silently dropped. `forget(tags=...)` also doesn't work because tags are never stored.

**Where to look:** `contextro-memory/src` — the `remember` handler needs to persist the tags field, and the `forget` handler needs to read the tags param from the request and filter by it.

---

### 5. Implement `code(list_symbols)` — **1 operation**

Returns "Unknown code operation" despite being a natural complement to `pattern_search`.

---

### 6. Fix `introspect` — **agent usability**

The tool description index is empty. Register tool names and descriptions into a searchable store at startup so `introspect` can surface them.

---

### 7. Improve `overview` and `architecture` content

Beyond graph fixes, `overview` should include language breakdown and top directories. `architecture` should include entry points and layer mapping.

---

### 8. Fix `explain` symbol disambiguation

When multiple symbols share a name, `explain` should resolve to the most central or most-called one, not the first match. Add a scoring step using call frequency or file position.

---

## What Is Genuinely Useful Right Now

Even in v0.2.0 with these gaps, the following provide real value to an agent today:

- **`search(mode=bm25)`** — fast, high-confidence, relevant symbol hits
- **`find_symbol`** — reliable fuzzy lookup
- **`dead_code`** — flags unused symbols across the codebase
- **`analyze`** — identifies large, complex files
- **`focus`** — compact symbol map for a file without dumping its full content
- **`audit`** — quick quality score and structural recommendations
- **`code(pattern_search)`** — AST-level grep across the whole codebase
- **`commit_history` / `commit_search`** — semantic git history search
- **`remember` / `recall`** — persisting decisions and conventions across sessions
- **`knowledge`** — indexing docs and notes alongside code
- **`session_snapshot` / `restore`** — agent re-entry without losing context
- **`repo_add` / `repo_status`** — multi-repo awareness
