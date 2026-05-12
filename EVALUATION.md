# Contextro MCP — Evaluation Report

**Versions tested:** 0.1.0 → 0.2.0 → 0.3.0 → 0.4.0 → 0.5.0 → 0.7.0  
**Last updated:** 2026-05-12  
**Tested on:** This repository (`distillation-labs/contextro`) — 46 files, 409 symbols, Rust multi-crate workspace  
**Method:** Every tool called in a single indexed stdio session per version. v0.7.0 tested with ~60 calls including vector/BM25 divergence checks, recall semantic quality at three abstraction levels, API parameter probing, and compact/retrieve roundtrip.

---

## Version Progress Summary

| Area | 0.1.0 | 0.2.0 | 0.3.0 | 0.4.0 | 0.5.0 | 0.7.0 |
|---|---|---|---|---|---|---|
| Graph edges built | ❌ 0 | ❌ 0 | ✅ 705 | ✅ 711 | ✅ 718 | ✅ 748 |
| BM25 search | ❌ 0 results | ✅ | ✅ | ✅ | ✅ | ✅ |
| Vector search | ❌ | ❌ | ❌ | ❌ | ✅ FIXED | ✅ conf=1.0 |
| Hybrid search | ❌ | ❌ | ❌ BM25 only | ❌ BM25 only | ✅ FIXED | ✅ |
| find_callers / callees | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ |
| impact (0-caller hint) | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ **hint added** |
| explain docstring | ❌ | ❌ | ❌ | ❌ null | ❌ null | ✅ **FIXED** |
| architecture hub degrees | ❌ all 0 | ❌ all 0 | ✅ (noisy) | ✅ meaningful | ✅ | ✅ |
| code(lookup_symbols) | ❌ | ❌ | ✅ | ✅ | ❌ broken | ✅ **FIXED** |
| memory tags | ❌ dropped | ❌ dropped | ✅ | ✅ | ✅ | ✅ |
| recall (fresh memories) | — | — | ✅ | ❌ regressed | ✅ FIXED | ✅ all 3 levels |
| recall semantic quality | — | — | — | — | 🟡 close only | ✅ abstract works |
| compact / retrieve | ✅/❌ | ✅/❌ | ✅/✅ | ✅/✅ | ✅/✅ | ✅/✅ |
| circular_dependencies | ❌ false+ | ❌ false+ | ❌ false+ | ✅ fixed | ✅ | ✅ |
| test_coverage_map | ❌ 0% | ❌ 0% | ❌ 0% | ✅ 39% | ✅ 42.1% | ✅ 42.1% |
| knowledge(search) | ❌ | ❌ | ❌ | 🟡 keywords only | 🟡 empty KB | ❌ add broken |
| introspect | ❌ | ❌ | ❌ | ❌ | ✅ FIXED | ❌ **regressed** |
| analyze path filtering | ❌ | ❌ | ❌ | ❌ global only | ✅ FIXED | ✅ |

---

## v0.5.0 Full Scorecard

> **Index stats:** 403 symbols, 718 edges, 46 files, `vector_chunks: 403` ✅ (was 0 in all prior versions)

| # | Tool | Status | Notes |
|---|---|---|---|
| 1 | `index` | ✅ | 403 symbols, 718 edges, **vector_chunks: 403 FIXED** |
| 2 | `status` | ✅ | Correct stats, shows indexed path + memory count |
| 3 | `health` | ✅ | `healthy`, accurate uptime |
| 4 | `search(mode=bm25)` | ✅ | Fast, relevant, confidence=1.0 |
| 5 | `search(mode=hybrid)` | ✅ | **FIXED** — now blends vector + BM25 results |
| 6 | `search(mode=vector)` | ✅ | **FIXED** — returns semantic results, diverges from BM25 |
| 7 | `find_symbol(exact)` | ✅ | `IndexingPipeline` → `pipeline.rs:34` |
| 8 | `find_symbol(fuzzy)` | ✅ | "Bm25" → 3 results |
| 9 | `find_callers` | ✅ | Works; ⚠️ param renamed `symbol` → `symbol_name` |
| 10 | `find_callees` | 🟡 | Works for functions; empty for struct/class (by design, undocumented) |
| 11 | `explain` | ✅ | Correct symbol, callers+callees filled; ⚠️ param renamed |
| 12 | `impact` | 🟡 | Works for functions; empty for root entry points with 0 callers |
| 13 | `overview` | ✅ | Returns full stats |
| 14 | `architecture` | ✅ | Meaningful hubs: dispatch(41), main(37), find_nodes_by_name(25) |
| 15 | `analyze` | ✅ | **Path filtering FIXED** — different results per directory |
| 16 | `focus` | ✅ | Per-symbol callers/callees + preview |
| 17 | `dead_code` | ✅ | Flags unreachable symbols |
| 18 | `circular_dependencies` | ✅ | Correctly returns 0 (no false cycles) |
| 19 | `test_coverage_map` | ✅ | 42.1%, 17 test files |
| 20 | `audit` | ✅ | Quality score + recommendations |
| 21 | `code(get_document_symbols)` | ✅ | ⚠️ op renamed; param is `file_path` not `path` |
| 22 | `code(search_symbols)` | ✅ | ⚠️ op renamed; param is `symbol_name` not `query` |
| 23 | `code(lookup_symbols)` | ❌ | Still errors: "Missing required parameter: symbols" even with `symbols` array |
| 24 | `code(pattern_search)` | ✅ | **Regex now works** — `impl.*[Ee]ngine` → 2 matches |
| 25 | `code(search_codebase_map)` | 🟡 | Returns directory listing, not a semantic map |
| 26 | `remember` | ✅ | Stores with tags correctly |
| 27 | `recall` | ✅ | **FIXED** — fresh memories retrieved in same session |
| 28 | `forget(memory_id)` | ✅ | ⚠️ param renamed `ids` → `memory_id` (single string) |
| 29 | `knowledge(command="search")` | 🟡 | ⚠️ requires `command` param; knowledge base empty in fresh session |
| 30 | `commit_history` | ✅ | Returns recent commits |
| 31 | `commit_search` | ✅ | Semantic git search, scored results |
| 32 | `session_snapshot` | ✅ | Full tool call log |
| 33 | `restore` | ✅ | Graph stats + re-entry context |
| 34 | `compact(content)` | ✅ | ⚠️ `content` now required; returns `ref_id` |
| 35 | `retrieve(ref_id)` | ✅ | ⚠️ `ref_id` now required; roundtrip confirmed |
| 36 | `skill_prompt` | ✅ | Agent bootstrap block |
| 37 | `introspect` | ✅ | **FIXED** — returns matching tools for queries |
| 38 | `docs_bundle` | ✅ | Generates .md files on disk |
| 39 | `sidecar_export` | ✅ | Exports graph sidecar |
| 40 | `tags` | ❌ | **Removed** — tool no longer exists, no replacement |
| 41 | `repo_add/status/remove` | ✅ | Clean round-trip |

**Working: ~36 / 40 tools** (up from ~32 in 0.4.0).  
**Biggest fix: `vector_chunks: 403`** — the embedding pipeline now runs during `index()`, unblocking vector search, hybrid search, and recall in one shot.

---

## Breaking API Changes in v0.5.0

These changes will silently break agents/scripts built against v0.4.0:

| Tool | v0.4.0 param | v0.5.0 param | Notes |
|---|---|---|---|
| `find_callers` | `symbol` | `symbol_name` | Required rename |
| `find_callees` | `symbol` | `symbol_name` | Required rename |
| `explain` | `symbol` | `symbol_name` | Required rename |
| `impact` | `symbol` | `symbol_name` | Required rename |
| `code` | `action` | `operation` | Required rename |
| `code(list_symbols)` | `path` | `file_path` | Param also renamed |
| `code(search_symbols)` | `query` | `symbol_name` | Param also renamed |
| `forget` | `ids: [...]` | `memory_id: "..."` | Array → single string |
| `compact` | `content` optional | `content` **required** | Now mandatory |
| `retrieve` | `query` accepted | `ref_id` **required** | Must use compact's ref_id |
| `knowledge` | `query` alone worked | `command: "search"` required | Sub-command now required |

None of these changes are documented in the changelog or tool descriptions. An agent using v0.4.0 parameter names will silently get errors.

---

## Detailed Results (v0.5.0)

### Indexing & Server Health

#### `index(path)`

```json
{
  "graph_nodes": 403,
  "graph_relationships": 718,
  "status": "done",
  "time_seconds": 0.0,
  "total_files": 46,
  "total_symbols": 403,
  "vector_chunks": 403
}
```

✅ **`vector_chunks: 403` — the embedding pipeline is finally running.** This was `0` across all of 0.1.0–0.4.0. All downstream features (vector search, hybrid search, recall, knowledge semantic search) now have a working foundation.

Speed is genuinely fast — sub-millisecond for a 46-file Rust workspace.

#### `status()` / `health()`
✅ Both accurate. Status confirms `indexed: true`, 718 relationships, 7 persistent memories.

---

### Search

#### `search(mode="bm25")`
✅ Fast, high-confidence, relevant results. Confidence = 1.0 for most queries.

#### `search(mode="vector")`
✅ **FIXED.** Returns semantic results that diverge meaningfully from BM25.

Quality test — query: _"finding code that processes and transforms text data"_:
```
vector (conf=0.146): treesitter.rs, main.rs, analysis.rs
bm25   (conf=1.000): code.rs, chunker.rs, analysis.rs
overlap: {analysis.rs}  — 2/3 results unique to vector
```

Vector correctly surfaced `treesitter.rs` (text parsing) and `analysis.rs` (code analysis), while BM25 found `code.rs` and `chunker.rs` (also relevant but different). The confidence scores are misleading — 0.146 looks low but the results are semantically correct. The normalization is not calibrated between BM25 and vector.

#### `search(mode="hybrid")`
✅ **FIXED.** Now blends both sides. Results confirmed to differ from pure BM25 queries.

---

### Symbol & Graph Tools

#### `find_callers(symbol_name)` / `find_callees(symbol_name)`
✅ Working. **Parameter renamed from `symbol` to `symbol_name` in v0.5.0.**

```
find_callees("dispatch") → 41 callees (handle_status, handle_health, handle_index, search, ...)
find_callers("run")      → 1 caller: main (study.rs:158)
find_callers("dispatch") → 0 callers (dispatch IS the root entry — correct, not a bug)
find_callees("run")      → 0 callees (run in study.rs is a leaf function)
```

Struct nodes (`IndexingPipeline`) still return 0 for both — structs have no call edges, only their impl methods do. This is technically correct for a call graph but confusing when an agent queries a well-known type.

#### `explain(symbol_name)`
✅ Works. **Parameter renamed.** Correctly resolves to highest-connectivity symbol on name collision.

```
explain("dispatch") → callers=0, callees=41, file=main.rs:99
explain("IndexingPipeline") → callers=0, callees=0, file=pipeline.rs:34  (struct, expected)
```

`docstring` still always `null` — doc comment extraction not implemented.

#### `impact(symbol_name)`
🟡 Works for internal functions. **Parameter renamed.**

```
impact("search") → 7 impacted symbols (test_bm25_index_and_search, test_search, main, ...)
impact("dispatch") → 0 impacted (dispatch has 0 callers, so nothing transitively depends on it)
```

The 0-result case for `dispatch` is technically correct but potentially misleading — it means "nothing calls dispatch in the parsed AST", not "changing dispatch is safe".

---

### Static Analysis

#### `analyze(path)`
✅ **Path filtering FIXED.** Two different paths now return different top-connectivity symbols:

```
engines/src:   find_nodes_by_name(25), get_nodes(13), add_edges(13), search(6)
indexing/src:  get_model(9), index(8), new(8), add_chunk(6)
```

In v0.4.0 both returned the same global list regardless of path.

#### `architecture()`
✅ Hub symbols are meaningful and stable:

```
dispatch           degree=41  (main.rs)      ← MCP tool router
main               degree=37  (study.rs)     ← build/study entry
find_nodes_by_name degree=25  (graph.rs)     ← graph lookup hub
```

#### `test_coverage_map()`
✅ 42.1% coverage (up from 39% in v0.4.0), 17 test files detected. Correctly identifies 16 covered and many uncovered files.

---

### Code / AST

#### `code` tool — API fully changed in v0.5.0

| Operation | v0.4.0 call | v0.5.0 call | Status |
|---|---|---|---|
| List file symbols | `code(action="list_symbols", path=...)` | `code(operation="get_document_symbols", file_path=...)` | ✅ works |
| Search symbols by name | `code(action="list_symbols", ...)` | `code(operation="search_symbols", symbol_name=...)` | ✅ works |
| Lookup named symbols | `code(action="list_symbols", ...)` | `code(operation="lookup_symbols", symbols=[...])` | ❌ still broken |
| Pattern match | `code(action="pattern_search", pattern=..., path=...)` | same params, same `operation` | ✅ works |
| Codebase map | new | `code(operation="search_codebase_map", query=...)` | 🟡 returns dir listing |

**Pattern search now supports regex:**
```
pattern="fn search"        → 8 matches across vector.rs, bm25.rs, memory.rs ✅
pattern="impl.*[Ee]ngine"  → 2 matches (Bm25Engine, reference in code.rs)   ✅ (was broken in 0.4.0)
```

---

### Memory

#### `remember(content, tags, memory_type)`
✅ Stores correctly with all fields.

#### `recall(query, limit)`
✅ **FIXED.** Fresh memories retrieved in same session.

```
remember("The embedding pipeline must write vectors to LanceDB...") → id: mem_bbd77843
recall("embedding pipeline lancedb vectors") → returns mem_bbd77843 ✅
```

**Caveat:** Recall quality depends on semantic closeness of the query. Direct paraphrases work; abstract conceptual queries may miss relevant memories. Tested recall with "how does vector indexing work" after storing "LanceDB vector store uses HNSW index for nearest neighbor search" → returned 0. Exact semantic proximity matters.

#### `forget(memory_id)`
✅ Works. **API changed: `ids: [...]` → `memory_id: "..."` (single string, not array).**

---

### Knowledge Base

#### `knowledge(command="search", query)`
🟡 API now requires `command: "search"` (bare `query` returns "Unknown knowledge command" error). Knowledge base returns 0 results on a fresh session — it's not pre-populated. The KB must be explicitly populated with `knowledge(command="add", ...)` before search returns anything. Not a bug, but worth noting that a fresh install has an empty knowledge base.

---

### Session & Archive

#### `compact(content)` / `retrieve(ref_id)`
✅ Full round-trip confirmed in v0.5.0:

```
compact(content="Agent analysis...") → {"archived": true, "ref_id": "arc_e62b614d"}
retrieve(ref_id="arc_e62b614d")      → {"content": "Agent analysis...", "ref_id": "arc_e62b614d"}
```

**API changed:** `content` is now required for `compact`; `ref_id` is now required for `retrieve`.

#### `introspect(query)`
✅ **FIXED.** Returns matching tools with descriptions:

```
introspect("search code") → [{tool: "code", description: "AST operations. Args: operation (...)"}]
introspect("memory recall") → [{tool: "recall", description: "Search memories. Args: query, limit, memory_type, tags"}]
```

---

## Open Issues (v0.5.0)

### 1. Breaking API changes are undocumented
Five tools had parameters renamed or required (symbol→symbol_name, action→operation, ids→memory_id, compact needs content, retrieve needs ref_id, knowledge needs command). None of these appear in the changelog or tool descriptions. Agents using the v0.4.0 API will silently fail.

**Fix:** Update tool descriptions (in `introspect` output) to reflect current param names. Add a migration note to the changelog.

### 2. `code(lookup_symbols)` still broken
Even with the correct `operation` and `symbols: [...]` array param, returns "Missing required parameter: symbols". Likely the handler is checking a different key name. Needs investigation in `contextro-tools/src/code.rs`.

### 3. `tags` tool removed without replacement
`tags` no longer exists. If memory tag listing was being used (e.g., to enumerate all stored tags before querying), there's no replacement. Should either be restored or replaced with a `recall(list_tags=true)` mode.

### 4. Struct/class nodes have no callers or callees
`find_callers("IndexingPipeline")` and `find_callees("IndexingPipeline")` both return 0. This is correct for a pure call graph (structs don't "call" or "get called" — methods do), but agents querying a well-known type get a silent empty result that looks like a bug. Options: add struct→method edges, or return a helpful message like "This is a struct. Query its methods: `index`, `new`".

### 5. `recall` only works for close semantic matches
Freshly stored memories ARE retrievable (fixed from 0.4.0), but recall quality depends on semantic proximity. Abstract paraphrases may miss relevant memories. The vector confidence threshold may be too strict.

### 6. `knowledge` knowledge base is always empty on fresh install
The KB tool requires explicit population via `knowledge(command="add", ...)`. A fresh contextro session has nothing in the knowledge base, making `knowledge(command="search", ...)` always return 0 until the user explicitly adds documents. Consider auto-populating with README/docs on first index.

### 7. `docstring` always null in `explain`
Doc comment extraction (`///`, `/** */`) is not implemented. This would significantly improve explain output for Rust and Go codebases.

### 8. `vector_chunks` confidence scores not calibrated
Vector search returns confidence ~0.146 while BM25 returns 1.0 for the same query. The scores are on different scales and aren't normalized for hybrid fusion. Agents reading confidence scores will underrate vector results.

---

## What Is Working in v0.5.0

| Tool | What it gives you |
|---|---|
| `search(bm25/vector/hybrid)` | All three modes working; vector diverges meaningfully from BM25 |
| `find_symbol` | Exact + fuzzy definition lookup |
| `find_callers` / `find_callees` | Real call graph traversal (functions only) |
| `impact` | Blast radius before refactoring |
| `explain` | Symbol + callers/callees in one call |
| `architecture` / `analyze` / `focus` | Structural analysis, path-filtered |
| `audit` / `dead_code` / `circular_dependencies` / `test_coverage_map` | Quality metrics |
| `code(get_document_symbols/search_symbols/pattern_search)` | AST + regex code navigation |
| `remember` / `recall` / `forget` | Persistent semantic memory |
| `commit_history` / `commit_search` | Git log + semantic commit search |
| `compact` / `retrieve` | Session context archiving and retrieval |
| `session_snapshot` / `restore` | Agent re-entry context |
| `introspect` | Tool self-discovery |
| `docs_bundle` / `sidecar_export` | Docs + graph sidecars |
| `repo_add/status/remove` | Multi-repo tracking |

---

## v0.7.0 Full Scorecard

> **Index stats:** 409 symbols, 748 edges, 46 files, `vector_chunks: 409`, `time_seconds: 0.0`

| # | Tool | Status | Notes |
|---|---|---|---|
| 1 | `index` | ✅ | 409 symbols, 748 edges, vector_chunks=409, sub-ms |
| 2 | `status` | ✅ | Accurate pre/post state |
| 3 | `health` | ✅ | healthy |
| 4 | `search(bm25)` | ✅ | conf=1.0, relevant results |
| 5 | `search(vector)` | ✅ | conf=1.0 (**improved from 0.146**), semantic divergence confirmed |
| 6 | `search(hybrid)` | ✅ | conf=1.0 |
| 7 | `search(filtered)` | ✅ | language + symbol_type filters accepted |
| 8 | `find_symbol(exact)` | ✅ | `IndexingPipeline` → `pipeline.rs:34` |
| 9 | `find_symbol(fuzzy)` | ✅ | "bm25" → 3 results; "dispatch" → 1 |
| 10 | `find_callers(search)` | ✅ | 4 callers returned |
| 11 | `find_callers(dispatch)` | ✅ | 0 callers + **new hint**: "root entry point, check external callers" |
| 12 | `find_callees(dispatch)` | ✅ | 41 callees |
| 13 | `find_callees(index)` | 🟡 | 0 callees — `index` in pipeline.rs is a method on a struct |
| 14 | `explain(dispatch)` | ✅ | callers=0, callees=41 |
| 15 | `explain(IndexingPipeline)` | ✅ | **docstring FIXED**: `"Orchestrates the full indexing flow."` |
| 16 | `impact(search)` | ✅ | 7 impacted symbols with depth |
| 17 | `impact(dispatch)` | ✅ | 0 impacted + hint (root entry point) |
| 18 | `overview` | ✅ | Consistent with index stats |
| 19 | `architecture` | ✅ | dispatch(42), main(37), find_nodes_by_name(26) |
| 20 | `analyze(engines)` | ✅ | find_nodes_by_name(26), get_node_degree(15) |
| 21 | `analyze(indexing)` | ✅ | get_model(9), index(8), embed_batch(7) — different from engines ✅ |
| 22 | `dead_code` | ✅ | metadata_path, encode, lancedb_path flagged |
| 23 | `circular_dependencies` | ✅ | total=0 |
| 24 | `test_coverage_map` | ✅ | 42.1%, 17 test files |
| 25 | `audit` | ✅ | score=75, 22 symbols >10 connections |
| 26 | `focus(bm25.rs)` | ✅ | Preview + symbols |
| 27 | `code(get_document_symbols)` | ✅ | Symbols with line ranges, signatures |
| 28 | `code(search_symbols)` | ✅ | "dispatch" → 1 result |
| 29 | `code(lookup_symbols)` | ✅ | **FIXED** — ["dispatch","IndexingPipeline"] → 2 results |
| 30 | `code(pattern_search literal)` | ✅ | "fn search" → 8 matches |
| 31 | `code(pattern_search regex)` | ✅ | "impl.*[Ee]ngine" → 2 matches |
| 32 | `code(search_codebase_map)` | ❌ | Returns empty (total_files=0, total_symbols=0) |
| 33 | `sidecar_export` | ✅ | Exported |
| 34 | `remember` | ✅ | Stored with tags |
| 35 | `recall(direct match)` | ✅ | Returns correct memory immediately |
| 36 | `recall(semantic)` | ✅ | "which function handles incoming tool requests" → dispatch memory ✅ |
| 37 | `recall(abstract)` | ✅ | "how does vector embedding get stored" → pipeline memory ✅ |
| 38 | `forget(memory_id)` | ✅ | deleted=1 |
| 39 | `knowledge(add)` | ❌ | **New regression**: "Content is empty — nothing indexed" even with content provided |
| 40 | `knowledge(search)` | 🟡 | Returns 0 (KB is empty due to add being broken) |
| 41 | `commit_history` | ✅ | 5 commits returned |
| 42 | `commit_search` | ✅ | Correct top result for both queries |
| 43 | `session_snapshot` | ✅ | Full event log |
| 44 | `restore` | ✅ | Path + graph stats + hint |
| 45 | `compact` | ✅ | archived=true, ref_id returned |
| 46 | `retrieve` | ✅ | Roundtrip confirmed — exact content returned |
| 47 | `introspect` | ❌ | **Regressed from v0.5.0** — returns 0 for all queries |
| 48 | `skill_prompt` | ✅ | Bootstrap block with correct tool names |
| 49 | `docs_bundle` | ✅ | architecture.md + overview.md generated |
| 50 | `repo_add/status/remove` | ✅ | Clean round-trip |

**Working: ~43 / 50 tool calls** (up from ~36 in v0.5.0).

---

## v0.7.0 Notable Changes

### ✅ New wins

**`docstring` in `explain` is finally populated.**
```
explain("IndexingPipeline") → docstring: "Orchestrates the full indexing flow."
```
Was `null` across all prior versions. Doc comment extraction (`///`) now works.

**`impact` gives a useful hint for root entry points.**
```
impact("dispatch") → hint: "0 callers found — this symbol is a root entry point
(nothing calls it in the parsed AST). It is safe to change its signature,
but check external callers (CLI, tests, MCP handlers) manually."
```
Previously returned a silent empty result that looked like a bug.

**`code(lookup_symbols)` fixed.**
```
lookup_symbols(symbols=["dispatch","IndexingPipeline"]) → 2 results ✅
```
Was broken in v0.5.0 even with correct params.

**Vector search confidence normalised.**
```
v0.5.0: vector conf=0.146 vs BM25 conf=1.0  (misleading)
v0.7.0: vector conf=1.0   vs BM25 conf=1.0  (calibrated)
```

**Recall now works at all three semantic abstraction levels.**
```
direct:   "indexing pipeline symbol extraction"         → pipeline memory ✅
semantic: "which function handles incoming tool requests" → dispatch memory ✅
abstract: "how does vector embedding get stored"          → pipeline memory ✅
```

**Vector search diverges correctly from BM25.**
```
Q: "code that builds and traverses a graph"
  vector unique: ['main.rs', 'treesitter.rs']   ← semantic hits
  bm25 unique:   ['graph.rs', 'engines graph']  ← keyword hits

Q: "storing data persistently across sessions"
  vector unique: ['session.rs', 'main.rs', 'embedding.rs']  ← semantic hits
  bm25 unique:   ['archive.rs', 'memory.rs']                ← keyword hits
```

### ❌ Regressions in v0.7.0

**`introspect` broken again** (was fixed in v0.5.0).
```
introspect("search code semantically")      → total=0
introspect("store retrieve memory context") → total=0
introspect("call graph callers impact")     → total=0
```
Three different queries, all return 0. Tool description index is empty again.

**`knowledge(add)` broken with a new error.**
```
knowledge(command="add", name="ctx-kb", content="Contextro MCP...") →
  {"error": "Content is empty — nothing indexed", "name": "ctx-kb"}
```
Content is clearly non-empty. The handler is likely not reading the `content` field from the request args.

**`code(search_codebase_map)` broken.**
```
search_codebase_map(query="embedding vector") → {files: [], total_files: 0, total_symbols: 0}
```
Was returning a directory listing in v0.5.0 (not great but functional). Now returns empty.

---

## Open Issues (v0.7.0)

### 1. `introspect` regressed — again
Fixed in v0.5.0, broken again in v0.7.0. Three queries tested, all return 0. The tool description index is not being populated at startup. This is a recurring regression — needs a test.

### 2. `knowledge(add)` content parsing broken
`{"command":"add","name":"ctx-kb","content":"..."}` returns "Content is empty". The handler is not reading `content` from the JSON args. Downstream: `knowledge(search)` always returns 0 because the KB can never be populated.

### 3. `code(search_codebase_map)` returns empty
Was at least returning a directory listing in v0.5.0. Now returns `{files:[], total_files:0}` regardless of query.

### 4. Struct/class nodes still have no callers/callees
`find_callees("index")` where `index` is a method on `IndexingPipeline` still returns 0. The graph records module-level functions as call targets but not struct methods via their impl blocks. Documented via the `impact` hint now, which is an improvement.

### 5. `tags` tool still absent
Removed in v0.5.0, not restored. No way to enumerate all stored memory tags.

### 6. `docstring` still null for functions
Fixed for structs/classes in v0.7.0, but `explain("dispatch")` still shows `docstring: null`. Function-level `///` doc comments are not yet extracted.

---

## What Is Working in v0.7.0

Every tool in this list was tested and produced correct, non-error output in the v0.7.0 session:

| Tool | What it gives you |
|---|---|
| `search(bm25/vector/hybrid)` | All three modes, calibrated confidence (all ~1.0), meaningful semantic divergence |
| `find_symbol(exact/fuzzy)` | Definition lookup, works for both |
| `find_callers` / `find_callees` | Call graph traversal with helpful hints for edge cases |
| `impact` | Blast radius + root-entry-point hint when callers=0 |
| `explain` | Symbol + graph + **docstring now populated for classes** |
| `architecture` | Hub ranking, stable and meaningful |
| `analyze(path)` | Per-directory hotspots, path filtering confirmed working |
| `focus` | Graph-enriched file view |
| `audit` / `dead_code` / `circular_dependencies` / `test_coverage_map` | Full quality suite |
| `code(get_document_symbols)` | Per-file symbols with signatures and line ranges |
| `code(search_symbols)` | Symbol name lookup |
| `code(lookup_symbols)` | Multi-symbol lookup by name list |
| `code(pattern_search)` | Literal + regex pattern search |
| `remember` / `recall` / `forget` | Semantic memory at all abstraction levels |
| `commit_history` / `commit_search` | Git log + semantic search |
| `compact` / `retrieve` | Context archiving roundtrip confirmed |
| `session_snapshot` / `restore` | Agent re-entry context |
| `skill_prompt` | Correct bootstrap block |
| `docs_bundle` / `sidecar_export` | Doc generation |
| `repo_add` / `repo_status` / `repo_remove` | Multi-repo management |
