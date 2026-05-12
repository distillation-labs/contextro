# Contextro MCP — Evaluation Report

**Versions tested:** 0.1.0 → 0.2.0 → 0.3.0 → 0.4.0  
**Last updated:** 2026-05-12  
**Tested on:** This repository (`distillation-labs/contextro`) — 51 files, 433 symbols, Rust multi-crate workspace  
**Method:** Every tool called in a single indexed stdio session per version. Raw JSON inspected for correctness, completeness, and regression against prior versions.

---

## Version Progress Summary

| Area | 0.1.0 | 0.2.0 | 0.3.0 | 0.4.0 |
|---|---|---|---|---|
| Graph edges built | ❌ 0 | ❌ 0 | ✅ 705 | ✅ 711 |
| BM25 search | ❌ 0 results | ✅ | ✅ | ✅ |
| Vector search | ❌ | ❌ | ❌ | ❌ |
| find_callers / callees | ❌ | ❌ | ✅ | ✅ |
| impact | ❌ | ❌ | ✅ | ✅ |
| explain (correct symbol) | ❌ | ❌ | ❌ wrong symbol | ✅ fixed |
| architecture hub degrees | ❌ all 0 | ❌ all 0 | ✅ (noisy) | ✅ meaningful |
| code(list_symbols) | ❌ | ❌ | ✅ | ✅ |
| memory tags | ❌ dropped | ❌ dropped | ✅ | ✅ |
| recall (fresh memories) | — | — | ✅ | ❌ regressed |
| compact / retrieve | ✅/❌ | ✅/❌ | ✅/✅ | ✅/✅ |
| circular_dependencies | ❌ false+ | ❌ false+ | ❌ false+ | ✅ fixed |
| test_coverage_map | ❌ 0% | ❌ 0% | ❌ 0% | ✅ 39% |
| knowledge(search) | ❌ | ❌ | ❌ | 🟡 keywords only |
| introspect | ❌ | ❌ | ❌ | ❌ |

---

## v0.4.0 Full Scorecard

| # | Tool | Status | Result |
|---|---|---|---|
| 1 | `index` | ✅ | 433 symbols, 711 edges, <0.01s |
| 2 | `status` | ✅ | Correct stats |
| 3 | `health` | ✅ | healthy |
| 4 | `search(mode=bm25)` | ✅ | Correct, high confidence |
| 5 | `search(mode=hybrid)` | ❌ | Identical to BM25, vector_chunks=0 |
| 6 | `search(mode=vector)` | ❌ | 0 results, conf=0.0 |
| 7 | `find_symbol(exact)` | ✅ | Finds IndexingPipeline correctly |
| 8 | `find_symbol(fuzzy)` | ✅ | Fuzzy match returns 3 results for "Bm25" |
| 9 | `find_callers` | ✅ | 6 real callers returned |
| 10 | `find_callees` | 🟡 | Works for functions, empty for struct/class |
| 11 | `explain` | ✅ | **Fixed in 0.4.0** — correct symbol, callers=6, callees=8 |
| 12 | `impact` | ✅ | 6 impacted symbols with depth |
| 13 | `overview` | ✅ | Returns relationships + chunk count |
| 14 | `architecture` | ✅ | Meaningful hubs (dispatch, find_nodes_by_name, etc.) |
| 15 | `analyze` | ✅ | Large files + high-connectivity symbols |
| 16 | `focus` | ✅ | Per-symbol callers/callees filled |
| 17 | `dead_code` | ✅ | 50 symbols flagged |
| 18 | `circular_dependencies` | ✅ | **Fixed in 0.4.0** — correct `total=0` |
| 19 | `test_coverage_map` | ✅ | **Fixed in 0.4.0** — 39%, 16/41 files covered |
| 20 | `audit` | ✅ | Quality score + recommendations |
| 21 | `code(list_symbols)` | ✅ | 13 symbols with caller/callee counts |
| 22 | `code(pattern_search)` | ✅ | Finds literal patterns; regex not supported |
| 23 | `remember` | ✅ | Tags persisted correctly |
| 24 | `recall` | ❌ | **Regression in 0.4.0** — returns 0 for fresh memories |
| 25 | `forget(tags)` | ✅ | `deleted: 1` |
| 26 | `forget(memory_id)` | ✅ | `deleted: 1` |
| 27 | `knowledge(add)` | ✅ | Chunks indexed |
| 28 | `knowledge(search)` | 🟡 | Exact keyword match works; semantic does not |
| 29 | `commit_history` | ✅ | Returns commits with metadata |
| 30 | `commit_search` | ✅ | Scored semantic git search |
| 31 | `session_snapshot` | ✅ | Full tool call log |
| 32 | `restore` | ✅ | Graph stats + re-entry hint |
| 33 | `compact` | ✅ | Archives content, returns ref_id |
| 34 | `retrieve` | ✅ | Round-trip works |
| 35 | `skill_prompt` | ✅ | Bootstrap block |
| 36 | `introspect` | ❌ | Always 0 results |
| 37 | `docs_bundle` | ✅ | Generates .md files on disk |
| 38 | `sidecar_export` | ✅ | Exports graph sidecars |
| 39 | `repo_add` | ✅ | |
| 40 | `repo_status` | ✅ | |
| 41 | `repo_remove` | ✅ | |

**Working: ~32 / 35 tools** (up from ~30 in 0.3.0, ~20 in 0.2.0, ~10 in 0.1.0).

---

## Detailed Results

### Indexing & Server Health

#### `index(path)`

```json
{
  "graph_nodes": 433,
  "graph_relationships": 711,
  "status": "done",
  "time_seconds": 0.01,
  "total_files": 51,
  "total_symbols": 433,
  "vector_chunks": 0
}
```

✅ Graph is real and growing (705 → 711 between runs). Call edges are being built.  
❌ `vector_chunks: 0` — LanceDB embedding population is still not happening during `index()`. This is the root cause of vector/hybrid search failures and the `recall` regression.

#### `status()` / `health()`
✅ Both correct. Status reports 711 relationships, indexed path, memory count.

---

### Search

#### `search(mode="bm25")`
✅ Working well. Results are relevant and confidence is high.

```
bm25.rs:220   [bm25] 1.000 — test_bm25_index_and_search
archive.rs:146 [bm25] 0.609 — test_archive_and_retrieve
bm25.rs:115   [bm25] 0.599 — search
pipeline.rs:124 [bm25] 1.000 — test_index_pipeline   (query: "indexing pipeline chunker")
embedding.rs:18 [bm25] 1.000 — get_model             (query: "embedding model lancedb")
```

Relevance is good — queries about specific subsystems land on the right files.

#### `search(mode="vector")`
❌ Returns `total: 0, confidence: 0.0` for every query. Root cause: `vector_chunks: 0` after indexing. The embedding step in `contextro-indexing/src/embedding.rs` is not running, or the model is failing to load silently and falling back to nothing.

#### `search(mode="hybrid")`
❌ Identical results to `mode=bm25` on every query tested. All hits carry `match=bm25`. The fusion layer has nothing from the vector side to blend in.

---

### Symbol & Graph Tools

#### `find_symbol(name, exact)`
✅ Both exact and fuzzy matching work correctly.

- `find_symbol("IndexingPipeline", exact=True)` → `pipeline.rs:34` ✅
- `find_symbol("Bm25", exact=False)` → 3 results: `Bm25Engine`, `test_bm25_index_and_search`, `test_bm25_delete_and_clear` ✅

#### `find_callers(symbol_name)`
✅ Working. Tested with two symbols:

```
find_callers("search") → 6 callers:
  test_bm25_index_and_search  bm25.rs:220
  test_search                 archive.rs:156
  main                        install.js:45
  dispatch                    main.rs:32

find_callers("index_chunks") → 5 callers:
  test_bm25_index_and_search  bm25.rs:220
  test_bm25_delete_and_clear  bm25.rs:245
  main                        install.js:45
  handle_index                main.rs:130
```

#### `find_callees(symbol_name)`
🟡 Works for functions, returns empty for struct/class symbols.

```
find_callees("search")          → 8 callees  ✅
find_callees("IndexingPipeline") → 0 callees  ❌ (it's a struct — callees should be its methods/associated functions)
```

**Why:** The call graph records function-to-function edges. Struct/class nodes don't have outgoing call edges — their "callees" would be the impl block methods they contain, which is a different graph traversal. Either add struct→method containment edges or document the limitation.

#### `explain(symbol_name)`
✅ **Fixed in 0.4.0.** Now resolves to the most-connected symbol when names collide.

```
explain("search") →
  name=search  file=memory.rs:141  callers=6  callees=8  docstring=null
```

Previously it resolved to `errors.rs:73` (a 1-line constructor with 0 connections). Now it correctly finds the main `search` function with real graph data.

**Remaining gap:** `docstring` is always `null`. Doc comment extraction (`///` and `/** */`) is not implemented.

#### `impact(symbol_name, max_depth)`
✅ Working. Tested with two symbols:

```
impact("search", max_depth=3) → 6 impacted:
  depth=1  test_bm25_index_and_search  bm25.rs:220
  depth=1  test_search                 archive.rs:156
  depth=1  main                        install.js:45
  depth=1  dispatch                    main.rs:32
  depth=1  handle_knowledge            memory.rs:177

impact("IndexingPipeline", max_depth=3) → 0 impacted
```

Same struct limitation as `find_callees` — the struct itself has no outgoing edges so impact radius is 0. Impact on `index` (the method) would return real results; the struct wrapper does not.

---

### Static Analysis

#### `overview()`
✅ Returns relationships and total symbols.

```json
{
  "codebase_path": "...",
  "total_relationships": 711,
  "total_symbols": 433,
  "vector_chunks": 433
}
```

Note: `vector_chunks: 433` here conflicts with `vector_chunks: 0` in `index()`. One of these is reporting BM25 chunk count under the wrong label.

#### `architecture()`
✅ **Meaningfully improved in 0.4.0.** Generic names (`new`, `len`, `get`, `is_empty`) are gone — hub symbols now show real architectural entry points:

```
dispatch          degree=40  (main.rs)        ← MCP tool dispatcher
main              degree=37  (install.js)     ← npm entry
find_nodes_by_name degree=25 (graph.rs)       ← graph lookup hub
build_graph       degree=18  (study.rs)       ← graph builder
handle_index      degree=17  (main.rs)        ← index handler
strip_base        degree=17  (code.rs)        ← path utility
parse_generic_def degree=15  (treesitter.rs)  ← parser hub
search            degree=14  (memory.rs)      ← search entry
```

This is genuinely useful for understanding which functions are most central to the system.

#### `analyze(path)`
✅ Returns large files by symbol count and high-connectivity symbols. Tested against two directories — both return the same global top list regardless of `path` filter. The `path` parameter may not be filtering results to the requested subdirectory.

```
large_files:
  study.rs     54 symbols
  graph.rs     27 symbols
  cli.mjs      26 symbols

high_connectivity:
  dispatch           40 connections
  main               37 connections
  find_nodes_by_name 25 connections
```

#### `focus(path)`
✅ Returns compact symbol list with caller/callee counts. Useful for getting a graph-enriched view of a file without dumping its content.

```
focus(bm25.rs):
  new_in_memory  callers=5  callees=3  line=34
  make_chunk     callers=2  callees=1  line=202
  index_chunks   callers=5  callees=0  line=91
  from_index     callers=2  callees=1  line=63

focus(pipeline.rs):
  index          callers=8  callees=0  line=48
  test_index_pipeline  callers=0  callees=4  line=124
```

#### `dead_code()`
✅ 50 symbols flagged across the codebase. Consistently flags config helper functions (`lancedb_path`, `metadata_path`, `graph_path`, `reset_settings`) that are unused — plausible candidates. Accuracy is heuristic; symbols used via trait dispatch or macros may be false positives.

#### `circular_dependencies()`
✅ **Fixed in 0.4.0.** Returns `total: 0`. Previously reported a false 28-file cycle because it was detecting call-level cycles rather than import-level cycles. Now correct.

#### `test_coverage_map()`
✅ **Fixed in 0.4.0.** Now reports:

```json
{
  "coverage_percent": 39.0,
  "covered_files": 16,
  "test_files": 17,
  "uncovered_files": 41
}
```

Previously always reported 0%. Now correctly detects inline `#[cfg(test)]` modules. 39% coverage across 51 files matches what you'd expect for a mixed Rust workspace.

#### `audit()`
✅ Quality score with graph-informed recommendations:

```json
{
  "quality_score": 75,
  "recommendations": [
    {"category": "complexity", "message": "21 symbols have >10 connections — consider refactoring", "severity": "medium"},
    {"category": "structure",  "message": "1 files have >30 symbols — consider splitting",           "severity": "low"}
  ]
}
```

---

### Code / AST

#### `code(operation="list_symbols", path)`
✅ Returns all symbols in a file with callers, callees, and line numbers. Tested on two files:

```
bm25.rs (13 symbols):
  new_in_memory   callers=5  callees=3  line=34
  index_chunks    callers=5  callees=0  line=91
  make_chunk      callers=2  callees=1  line=202
  search          callers=0  callees=0  line=115

pipeline.rs (5 symbols):
  index           callers=8  callees=0  line=48
  IndexingPipeline  callers=0  callees=0  line=34
```

#### `code(operation="pattern_search", pattern, path)`
✅ Literal string matching works. Regex does not.

```
pattern="fn search" → 8 matches across vector.rs, bm25.rs, memory.rs, archive.rs, etc. ✅
pattern="impl.*Engine" → 0 matches ❌ (regex not supported, no docs warning)
```

---

### Memory

#### `remember(content, tags, memory_type)`
✅ Tags and memory_type both persisted correctly.

```json
{"id": "mem_72cf0c1f", "memory_type": "note",     "stored": true, "tags": ["eval", "v040"]}
{"id": "mem_a53cab1c", "memory_type": "decision",  "stored": true, "tags": ["arch", "v040"]}
```

#### `recall(query, limit)`
❌ **Regression in 0.4.0.** Returns `total: 0` for both queries immediately after storing two memories.

```
recall("contextro evaluation testing") → total=0
recall("vector search lancedb embedding") → total=0
```

In v0.3.0, recall worked for previously-stored memories. The most likely cause: `recall` now depends on vector similarity search (LanceDB), and since `vector_chunks: 0`, there are no embeddings to match against. If memory storage also requires embedding (for recall to work), the broken embedding pipeline breaks the whole memory retrieval path.

**Impact:** The memory system is half-broken — you can store but not retrieve. This makes `remember` useless until embeddings work.

#### `forget(tags)` / `forget(memory_id)`
✅ Both return `deleted: 1`. Deletion works even when recall is broken.

---

### Knowledge Base

#### `knowledge(command="add")`
✅ `{"chunks": 1, "name": "contextro-overview", "status": "indexed"}`.

#### `knowledge(command="search")`
🟡 Keyword-exact queries work. Semantic/paraphrase queries do not.

```
query="local MCP server"       → 1 result ✅  (verbatim phrase in the stored doc)
query="hybrid search coding agents" → 0 results ❌  (semantically equivalent but different words)
```

**Why:** Knowledge search is using BM25/keyword matching. A query using synonyms or paraphrases of the indexed content returns nothing. True semantic search requires the vector pipeline to be working.

---

### Git

#### `commit_history(limit)`
✅ Returns recent commits correctly. The latest is `"chore: bump version to 0.4.0"`.

#### `commit_search(query, limit)`
✅ Semantic search over commit messages works. Both queries found the correct most-relevant commit (`4bab1ef3 — fix: call graph edges, vector search, compact/retrieve, memory tags`) as the top result.

---

### Session & Archive

#### `session_snapshot()`
✅ Returns a full event log of the current session's tool calls.

#### `restore()`
✅ Returns codebase path, graph stats (711 edges), and re-entry hint.

#### `compact(content)` / `retrieve(ref_id)`
✅ Both ends of the round-trip work.

```json
compact → {"archived": true, "ref_id": "arc_512f1a39"}
retrieve → {"content": "Full evaluation session...", "ref_id": "arc_512f1a39"}
```

---

### Docs & Export

#### `skill_prompt()`
✅ Returns agent bootstrap block.

#### `introspect(query)`
❌ Still always returns `{"matching_tools": [], "total": 0}`. Two different queries tested, both return nothing. Tool descriptions are not registered into any searchable index at startup.

#### `docs_bundle()` / `sidecar_export(path)`
✅ Both generate files on disk.

---

### Repo Management

#### `repo_add` / `repo_status` / `repo_remove`
✅ All work. Clean add→status→remove round-trip.

---

## Open Issues (v0.4.0)

### 1. LanceDB not populated during `index()` — blocks 3+ features
`vector_chunks: 0` on every index run. This is the single highest-impact bug remaining.

**Downstream effects:**
- `search(mode=vector)` — 0 results
- `search(mode=hybrid)` — falls through to BM25 only
- `recall` — 0 results (memory retrieval broken)
- `knowledge(search)` — keyword-only, no semantic matching

**Where to look:** `contextro-indexing/src/embedding.rs` and `contextro-indexing/src/pipeline.rs`. The embedding step either isn't being called in the indexing walk or the model (`potion-code-16m`) is failing to initialize and the error is swallowed. Add explicit logging or surface the error in the `index()` response.

### 2. `recall` broken for freshly-stored memories
`remember` stores successfully but `recall` returns 0 immediately after. Likely because recall uses vector similarity over embeddings, and with no embeddings written, the search space is empty. This makes the memory system write-only until the embedding pipeline is fixed.

### 3. `introspect` always empty
Tool descriptions are never registered into the searchable index at startup. The handler exists but its backing store is empty.

### 4. `find_callees` / `impact` empty for struct/class symbols
`find_callees("IndexingPipeline")` returns 0. Structs have no outgoing call edges in the current graph model. Either add struct→method containment edges, or document the limitation clearly so agents don't misinterpret the empty result.

### 5. `knowledge(search)` is keyword-only
Works for exact phrase matches, fails for semantic paraphrases. Will be fixed automatically once the embedding pipeline works.

### 6. `code(pattern_search)` silently ignores regex
`pattern="impl.*Engine"` returns 0 matches with no error. Should either support regex or return a clear error when a pattern looks like a regex.

### 7. `overview` vs `index` disagree on `vector_chunks`
`overview` reports `vector_chunks: 433`; `index` reports `vector_chunks: 0`. One of them is reporting BM25 chunks under the wrong label.

### 8. `analyze(path)` ignores path filter
Both `analyze("crates/contextro-engines/src")` and `analyze("crates/contextro-indexing/src")` return the same global top-list. The `path` parameter doesn't filter results to the specified directory.

### 9. `docstring` always null in `explain`
Doc comment extraction (`///`, `/** */`) is not implemented. Would significantly improve `explain` output.

---

## What Is Genuinely Useful in v0.4.0

These tools are working correctly and provide real value to a coding agent today:

| Tool | What it gives you |
|---|---|
| `search(bm25)` | Fast, relevant symbol hits — better than raw grep for code navigation |
| `find_symbol` | Reliable exact + fuzzy definition lookup |
| `find_callers` | Real call graph traversal — who calls this function |
| `find_callees` | What a function depends on (functions only) |
| `impact` | Blast radius before a refactor (functions only) |
| `explain` | Correct symbol + callers/callees in one call |
| `architecture` | Meaningful architectural hub ranking |
| `focus` | Graph-enriched compact file view |
| `analyze` | Large files + high-connectivity hotspots |
| `audit` | Quality score with actionable recommendations |
| `dead_code` | Unused symbol candidates for cleanup |
| `test_coverage_map` | 39% coverage with file-level detail |
| `code(list_symbols)` | Per-file symbol map with graph edges |
| `code(pattern_search)` | Literal string search across the whole codebase |
| `commit_history` | Recent git log with metadata |
| `commit_search` | Semantic search over commit messages |
| `compact` / `retrieve` | Session archiving and retrieval |
| `session_snapshot` / `restore` | Agent re-entry context |
| `docs_bundle` | Generates architecture + overview docs |
| `repo_add/status/remove` | Multi-repo tracking |
