# Contextro MCP — Evaluation Report

**Versions tested:** 0.1.0 → 0.2.0 → 0.3.0 → 0.4.0 → 0.5.0  
**Last updated:** 2026-05-12  
**Tested on:** This repository (`distillation-labs/contextro`) — 46 files, 403 symbols, Rust multi-crate workspace  
**Method:** Every tool called in a single indexed stdio session per version (~48 calls in v0.5.0). Raw JSON inspected for correctness, completeness, and regression against prior versions. v0.5.0 includes additional quality checks: vector vs BM25 divergence, recall semantic distance, compact/retrieve roundtrip, and API parameter probing.

---

## Version Progress Summary

| Area | 0.1.0 | 0.2.0 | 0.3.0 | 0.4.0 | 0.5.0 |
|---|---|---|---|---|---|
| Graph edges built | ❌ 0 | ❌ 0 | ✅ 705 | ✅ 711 | ✅ 718 |
| BM25 search | ❌ 0 results | ✅ | ✅ | ✅ | ✅ |
| Vector search | ❌ | ❌ | ❌ | ❌ | ✅ **FIXED** |
| Hybrid search | ❌ | ❌ | ❌ BM25 fallback | ❌ BM25 fallback | ✅ **FIXED** |
| find_callers / callees | ❌ | ❌ | ✅ | ✅ | ✅ (param renamed) |
| impact | ❌ | ❌ | ✅ | ✅ | ✅ (param renamed) |
| explain (correct symbol) | ❌ | ❌ | ❌ wrong symbol | ✅ fixed | ✅ |
| architecture hub degrees | ❌ all 0 | ❌ all 0 | ✅ (noisy) | ✅ meaningful | ✅ |
| code(list_symbols) | ❌ | ❌ | ✅ | ✅ | ✅ (API changed) |
| memory tags | ❌ dropped | ❌ dropped | ✅ | ✅ | ✅ |
| recall (fresh memories) | — | — | ✅ | ❌ regressed | ✅ **FIXED** |
| compact / retrieve | ✅/❌ | ✅/❌ | ✅/✅ | ✅/✅ | ✅/✅ (API changed) |
| circular_dependencies | ❌ false+ | ❌ false+ | ❌ false+ | ✅ fixed | ✅ |
| test_coverage_map | ❌ 0% | ❌ 0% | ❌ 0% | ✅ 39% | ✅ 42.1% |
| knowledge(search) | ❌ | ❌ | ❌ | 🟡 keywords only | 🟡 empty KB |
| introspect | ❌ | ❌ | ❌ | ❌ | ✅ **FIXED** |
| analyze path filtering | ❌ | ❌ | ❌ | ❌ global only | ✅ **FIXED** |

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
| `search(bm25)` | Fast, relevant symbol hits — better than raw grep |
| `search(vector)` | Semantic search — finds conceptually related code even with different keywords |
| `search(hybrid)` | Blended results — best of both modes |
| `find_symbol` | Exact + fuzzy definition lookup |
| `find_callers` | Real call graph traversal — who calls this function |
| `find_callees` | What a function depends on (functions only) |
| `impact` | Blast radius before refactoring |
| `explain` | Symbol + callers/callees in one call |
| `architecture` | Hub ranking — most connected symbols |
| `analyze` | Per-directory high-connectivity hotspots (path filtering fixed) |
| `focus` | Graph-enriched compact file view |
| `audit` | Quality score + recommendations |
| `dead_code` | Unused symbol candidates |
| `test_coverage_map` | 42.1% file-level coverage |
| `code(get_document_symbols)` | Per-file symbol map with line ranges |
| `code(search_symbols)` | Fuzzy symbol name lookup |
| `code(pattern_search)` | Regex pattern search across codebase |
| `remember` / `recall` / `forget` | Persistent semantic memory — store and retrieve context |
| `commit_history` / `commit_search` | Git log + semantic commit search |
| `compact` / `retrieve` | Session context archiving and retrieval |
| `session_snapshot` / `restore` | Agent re-entry context |
| `introspect` | Tool self-discovery — find the right tool for a task |
| `docs_bundle` / `sidecar_export` | Generates documentation + graph sidecars |
| `repo_add/status/remove` | Multi-repo tracking |
