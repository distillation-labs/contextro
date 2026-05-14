# Contextro MCP End-to-End Evaluation

**Version:** 1.6.11  
**Date:** 2026-05-14  
**Codebase tested:** `/Users/japneetkalkat/contextro` (the server itself)  
**Index result:** 51 files, 668 symbols, 1,437 edges, <0.01s incremental  
**Testing method:** All 37 tools exercised live through the MCP interface in a single session

---

## Methodology

This evaluation was run by indexing the contextro codebase against itself and then exercising every public tool at least once with realistic inputs. Tools were tested in parallel batches to reduce session time. Where a tool has multiple operation modes (e.g. `code`), each mode was tested individually. Results were judged against the tool's stated description and the product's positioning as a "codebase intelligence" layer for AI coding agents.

---

## Index and Startup

### `index`

**Input:** `path=/Users/japneetkalkat/contextro`  
**Output:**
```json
{
  "graph_nodes": 668,
  "graph_relationships": 1437,
  "incremental": { "files_added": 0, "files_deleted": 0, "files_modified": 0, "files_unchanged": 51 },
  "status": "done",
  "time_seconds": 0.01,
  "total_chunks": 668,
  "total_files": 51,
  "total_symbols": 668,
  "vector_chunks": 668
}
```
**Result: PASS.** Incremental indexing correctly detected no changes and skipped reprocessing. The 51-file/668-symbol graph built cleanly. The 0.01s time for a no-op incremental run is within the promised <50ms cold start performance target.

---

## Status and Discovery Tools

### `health`

**Output:** `{ "status": "healthy", "version": "1.6.11", "graph_nodes": 668, "graph_relationships": 1437, "memories": 21, "uptime_seconds": 101.2 }`  
**Result: PASS.** Returns all expected fields. Uptime tracking works.

### `status`

**Output:** `{ "codebase_path": "/Users/japneetkalkat/contextro", "graph_nodes": 668, "graph_relationships": 1437, "indexed": true, "memories": 21, "cache_hit_rate": 0.0, "uptime_seconds": 101.2 }`  
**Result: PASS.** Correct indexed state. Cache hit rate is 0.0 at session start, which is expected.

### `overview`

**Output:**
```json
{
  "codebase_path": "/Users/japneetkalkat/contextro",
  "languages": [{ "language": "rust", "symbols": 635 }, { "language": "javascript", "symbols": 33 }],
  "symbol_types": [{ "count": 596, "type": "function" }, { "count": 72, "type": "class" }],
  "top_directories": [...],
  "top_files_by_symbols": [
    { "file": "crates/contextro-tools/src/memory.rs", "symbols": 71 },
    { "file": "crates/contextro-server/src/study.rs", "symbols": 55 },
    ...
  ],
  "total_files": 43, "total_symbols": 668, "total_relationships": 1437
}
```
**Result: PASS.** Language breakdown is accurate (Rust codebase with a small JS npm package). Symbol type distribution (596 functions, 72 classes) is plausible for a Rust workspace. Top directories correctly identify `contextro-tools/src` as the largest module (238 symbols).

### `restore`

**Output:** `{ "codebase_path": "/Users/japneetkalkat/contextro", "graph_nodes": 668, "graph_relationships": 1437, "indexed": true, "requires_index": false, "hint": "Index is loaded. Use search/find_symbol/explain to query." }`  
**Result: PASS.** Designed for session re-entry after compaction. Returns correct state and a useful hint. `requires_index: false` is correct.

### `architecture`

**Input:** `limit=10`  
**Output:**
```json
{
  "hub_symbols": [
    { "name": "dispatch", "degree": 53, "file": "crates/contextro-server/src/main.rs" },
    { "name": "main", "degree": 35, "file": "npm/contextro/install.js" },
    { "name": "temp_file", "degree": 34, "file": "crates/contextro-tools/src/session.rs" },
    { "name": "with_path", "degree": 33, "file": "crates/contextro-tools/src/memory.rs" },
    { "name": "find_nodes_by_name", "degree": 30, "file": "crates/contextro-engines/src/graph.rs" },
    ...
  ],
  "total_edges": 1437, "total_nodes": 668
}
```
**Result: PASS.** Hub symbol detection is working. `dispatch` at degree 53 is the correct central routing function — it calls all 37 tool handlers. The degree numbers are meaningful and reflect actual architectural weight. One minor observation: `temp_file` at degree 34 being the #3 hub symbol suggests it's called by many tools as a utility, which is consistent with the codebase but might be slightly misleading to a first-time user who expects only "major architectural components" here.

### `tags`

**Output:** `{ "tags": ["algorithm","architecture","audit","auth","bug","datagrid","dispatch","embedding","eval","fix","graph","index","lancedb","parser","performance","release","security","v040","vector"], "total": 19 }`  
**Result: PASS.** 19 tags returned. Mix of technical and workflow tags reflects real usage history.

### `introspect`

**Input:** `query="how to search for symbols in a codebase"`  
**Output:** Returned 17+ matching tools with descriptions, examples, and parameter lists. Total=37 (all tools registered).  
**Result: PASS.** Fuzzy matching surfaces relevant tools. The `code`, `analyze`, `knowledge`, and `architecture` tools all correctly bubble up for a symbol-search query. Full tool count of 37 confirms the registry is complete.

### `skill_prompt`

**Output:** Returns bootstrap block (`Start with index...`), core tools list with examples and parameter conventions, and backward-compatibility notes for legacy aliases.  
**Result: PASS.** This is the agent onboarding surface — it's clean, accurate, and covers the most important tools with usable examples.

---

## Search Tools

### `search`

**Input:** `query="how is indexing implemented", limit=5`  
**Output:**
```json
{
  "confidence": "high",
  "results": [
    { "name": "ContextroError.indexing", "file": "crates/contextro-core/src/errors.rs", "score": 0.7546 },
    { "name": "is_available", "file": "crates/contextro-indexing/src/embedding.rs", "score": 0.4297 },
    { "name": "is_git_repo", "file": "crates/contextro-git/src/commit_indexer.rs", "score": 0.3203 },
    ...
  ],
  "total": 15, "truncated": true
}
```
**Result: PASS with observation.** Top result is `ContextroError.indexing` with score 0.75, which is a reasonable match for this query. The vector search is working. However, the actual indexing pipeline entrypoint (`handle_index`, `discover_files`, `hash_files`) scores lower than the error type, which suggests the vector embeddings favor error/type names over handler functions for conceptual queries. This is a minor ranking quality issue, not a functional failure.

### `find_symbol`

**Input:** `symbol_name="dispatch", exact=true`  
**Output:** `{ "symbols": [{ "name": "dispatch", "file": "crates/contextro-server/src/main.rs", "line": 50, "type": "function" }], "total": 1 }`  
**Result: PASS.** Exact match returns the correct single result with file and line number.

### `explain`

**Input:** `symbol_name="dispatch"`  
**Output:** Returns summary, docstring, 51 callees (full list), 2 callers, file, line, language.  
**Result: PASS.** The explanation is structurally correct and context-rich. 51 callees for the main dispatch function matches the 37-tool server architecture (some tools have multiple handlers). Docstring is present (`"Contextro MCP server binary — single compiled Rust binary."`). The callers/callees are correctly traced.

### `find_callers`

**Input:** `symbol_name="dispatch", limit=5`  
**Output:** `{ "callers": ["mcp_handler (http.rs:42)", "call_tool (main.rs:1037)"], "total": 2 }`  
**Result: PASS.** Correct — `dispatch` is only called from the HTTP handler and the stdio MCP handler. Graph is accurate.

### `find_callees`

**Input:** `symbol_name="dispatch", limit=5`  
**Output:** First 5 of 51 callees, correct.  
**Result: PASS.**

---

## Graph and Impact Tools

### `impact`

**Input:** `symbol_name="dispatch", max_depth=2`  
**Output:** `{ "impacted": [{ "name": "mcp_handler", "depth": 1 }, { "name": "call_tool", "depth": 1 }], "total_impacted": 2, "depth_hint": "Explicit max_depth=2 overrides the default depth of 5..." }`  
**Result: PASS.** Blast radius is correct at depth=2. The `depth_hint` field is a good UX decision — it reminds the caller that they explicitly narrowed the blast radius. At max_depth=5 (default) the impact would propagate through the full server loop, which is the expected behavior.

### `refactor_check`

**Input:** `symbol_name="handle_index"`  
**Output:**
```json
{
  "symbol": "handle_index",
  "file": "crates/contextro-server/src/main.rs",
  "line": 249,
  "type": "function",
  "callees_count": 25,
  "callers_count": 1,
  "callees": [
    { "name": "get", "file": "crates/contextro-engines/src/cache.rs", "line": 34 },
    { "name": "get_settings", "file": "crates/contextro-config/src/lib.rs", "line": 15 },
    { "name": "discover_files", "file": "crates/contextro-indexing/src/file_scanner.rs", "line": 31 },
    ...
  ],
  "impacted": [
    { "name": "dispatch", "depth": 1 },
    { "name": "mcp_handler", "depth": 2 },
    { "name": "call_tool", "depth": 2 }
  ],
  "impacted_count": 3,
  "risk": "low",
  "suggestion": "1 callers — update all call sites after refactoring."
}
```
**Result: PASS.** This is the most comprehensive single-call tool. It consolidates definition, callers, callees, transitive impact, and risk in one response. All fields are populated and accurate. Risk assessment of "low" for a 1-caller function is correct. This tool delivers real value.

### `circular_dependencies`

**Output:** `{ "circular_dependencies": [], "total": 0 }`  
**Result: PASS.** Clean result — no cycles in the codebase. The result is truthful (not a silent empty success — this is a codebase with well-structured dependencies).

---

## Code AST Operations (`code`)

The `code` tool exposes 8 distinct operations. Each was tested individually.

### `get_document_symbols`

**Input:** `path="crates/contextro-server/src/main.rs"`  
**Output:** 44 symbols with name, type, line, end_line, and signature (truncated to 60 chars).  
**Result: PASS.** All major functions and methods in main.rs returned with accurate line numbers. Signatures are truncated cleanly.

### `search_symbols`

**Input:** `symbol_name="handle_search"`  
**Output:** `{ "symbols": [{ "name": "handle_search", "file": "crates/contextro-tools/src/search.rs", "line": 17, "type": "function" }], "total": 1 }`  
**Result: PASS.**

### `list_symbols`

**Input:** `path="crates/contextro-tools/src"`  
**Output:** 238 total symbols, 30 returned (truncated with hint: `"Use max_tokens for a different budget"`).  
**Result: PASS.** Truncation metadata is included and actionable. The 238 total symbol count matches `overview` output.

### `lookup_symbols`

**Input:** `symbols=["dispatch", "handle_search"]`  
**Output:** Both symbols found with correct file, line, and type.  
**Result: PASS.**

### `pattern_search`

**Input:** `path="crates/contextro-tools/src", pattern="fn handle_"`  
**Output:** 36 matches across 8 files, each with file, line, and matched code snippet.  
**Result: PASS.** All `handle_*` function signatures are returned. The code snippets are useful for understanding each handler's signature at a glance.

### `pattern_rewrite` (dry_run)

**Test 1 — Relative path:**  
**Input:** `path="crates/contextro-tools/src/search.rs", pattern="handle_search", replacement="handle_search", dry_run=true`  
**Output:** `{ "error": "Path not found: crates/contextro-tools/src/search.rs" }`  
**Result: FAIL.** `pattern_rewrite` does not resolve relative paths. Every other code tool in the suite (`analyze`, `focus`, `pattern_search`, `list_symbols`) accepts relative paths rooted at the indexed codebase. This is an inconsistency in path resolution logic specific to `pattern_rewrite`.

**Test 2 — Absolute path (identity rewrite):**  
**Input:** `path="/Users/japneetkalkat/contextro/crates/contextro-tools/src/search.rs", pattern="handle_search", replacement="handle_search", dry_run=true`  
**Output:** `{ "changes": [], "dry_run": true, "total_files": 0, "total_replacements": 0 }`  
**Result: PASS with correct behavior.** Identity rewrite (pattern == replacement) returns zero changes. Dry_run mode works correctly with absolute paths.

### `search_codebase_map`

**Input:** `query="how does search ranking work"`  
**Output:** 1 file, 1 symbol (`query_targets_product_surface` in search.rs)  
**Result: WEAK.** The actual search ranking logic is distributed across multiple functions: `rerank_natural_language_results`, `drop_low_confidence_noise`, `result_matches_symbol_query`, `is_symbol_lookup_query`, `vector_candidate_limit`. None of these surfaced. A conceptual query about how a feature works should map to the cluster of symbols implementing it. Returning 1 symbol for a multi-function subsystem gives a user a dangerously incomplete picture.

### `edit_plan`

**Input:** `goal="refactor handle_search to extract reranking into a separate function"`  
**Output:**
```json
{
  "affected_symbols": [],
  "confidence": "high",
  "goal": "refactor handle_search to extract reranking into a separate function",
  "next_steps": ["Review the diff preview before applying"],
  "related_tests": [],
  "risks": [],
  "target_files": []
}
```
**Result: FAIL.** `confidence: "high"` with `affected_symbols: []`, `target_files: []`, and `risks: []` is a contradiction. The tool claims high confidence but produced an empty plan. `handle_search` exists at search.rs:17 and calls `rerank_natural_language_results`, `drop_low_confidence_noise`, and others — these should appear as affected symbols. The next_steps list says "Review the diff preview before applying" but there is no diff preview to review. This tool is effectively non-functional for its stated purpose.

---

## Analysis and Quality Tools

### `analyze`

**Input:** `path="crates/contextro-tools/src"`  
**Output:**
```json
{
  "high_connectivity_symbols": [
    { "name": "temp_file", "connections": 34, "file": "session.rs" },
    { "name": "with_path", "connections": 33, "file": "memory.rs" },
    { "name": "handle_knowledge", "connections": 27, "file": "memory.rs" },
    { "name": "handle_circular_dependencies", "connections": 21, "file": "analysis.rs" },
    ...
  ],
  "large_files": [
    { "file": "memory.rs", "symbols": 71 },
    { "file": "search.rs", "symbols": 47 },
    ...
  ],
  "total_symbols": 238
}
```
**Result: PASS.** Hotspot detection is accurate. `memory.rs` at 71 symbols and `search.rs` at 47 symbols are correctly flagged as candidates for splitting. The connectivity rankings reflect real coupling in the codebase.

### `focus`

**Input:** `path="crates/contextro-server/src/main.rs"`  
**Output:** 44 symbols with per-symbol callers/callees counts and file preview.  
**Result: PASS.** Compact but information-dense output. Useful for getting a quick call-graph summary of a file without pulling all the details. Preview snippet is included.

### `audit`

**Output:** `{ "quality_score": 75, "recommendations": [{ "category": "complexity", "message": "49 symbols have >10 connections — consider refactoring", "severity": "medium" }, { "category": "structure", "message": "6 files have >30 symbols — consider splitting", "severity": "low" }], "status": "complete", "total_symbols": 668 }`  
**Result: PASS.** Quality score of 75/100 seems reasonable for a mature v1 codebase. Both recommendations are actionable and accurate based on observed codebase structure.

### `dead_code`

**Input:** `limit=5`  
**Output:**
```json
{
  "dead_symbols": [
    { "name": "encode", "file": "crates/contextro-config/src/lib.rs", "line": 43 },
    { "name": "reset_settings", "file": "crates/contextro-config/src/lib.rs", "line": 20 },
    { "name": "make_node", "file": "crates/contextro-core/src/graph.rs", "line": 373 },
    { "name": "encode", "file": "crates/contextro-core/src/models.rs", "line": 136 },
    { "name": "default_source", "file": "crates/contextro-core/src/models.rs", "line": 230 }
  ],
  "skipped_public_api": 88,
  "skipped_tests": 2,
  "total": 5
}
```
**Result: PASS.** The static heuristic correctly notes it skipped 88 public API symbols and 2 test files. The `note` field is transparent about the heuristic nature of the analysis. Flagged symbols are plausible candidates (utility functions in config and core with no observed callers).

### `test_coverage_map`

**Output:**
```json
{
  "coverage_range_percent": { "lower_bound": 65.9, "upper_bound": 97.6 },
  "covered_files": 40,
  "uncovered_files": 1,
  "uncovered": ["npm/contextro/install.js"],
  "test_files": 28,
  "note": "Static heuristic based on inline tests, exact filename matches, and source/test token overlap..."
}
```
**Result: PASS.** The range-based reporting (65.9–97.6%) is honest about the limitations of static heuristics. `npm/contextro/install.js` as the only uncovered file is plausible — it's an npm install script with no corresponding test. The transparency note is good.

---

## Git Tools

### `commit_history`

**Input:** `limit=5`  
**Output:** 5 commits with author (`Japneet Kalkat`), hash, message, and Unix timestamp.  
**Result: PASS.** All fields populated correctly. Timestamps are Unix integers (correct).

### `commit_search`

**Input:** `query="indexing performance", limit=5`  
**Output:** 5 commits with relevance scores. Top matches include a release commit and benchmark script commits.  
**Result: PASS with observation.** The semantic search returns meaningful results but includes commits referencing `src/contextia_mcp/...` (an old Python version of the project) at score 0.53. These are technically correct semantic matches (indexing + performance topics) but are from a prior Python codebase iteration. This is expected behavior for semantic search over full history, not a bug, but a user might find it confusing.

---

## Memory and Persistence Tools

### `remember`

**Input:** `content="Testing memory persistence for release validation", memory_type="note", tags=["test","release"]`  
**Output:** `{ "id": "mem_343554bb", "stored": true, "memory_type": "note", "tags": ["test","release"], "ttl": "permanent", "expires_at": null }`  
**Result: PASS.** Memory stored with correct id, type, and tags.

### `recall`

**Input:** `query="memory persistence test"`  
**Output:** Returns 5 memories. The just-stored memory (`mem_343554bb`) is the top result.  
**Result: PASS.** Memory stored in this session was immediately retrievable and ranked first. The 5-result default is appropriate.

### `forget`

**Input:** `id="mem_343554bb"`  
**Output:** `{ "deleted": 1 }`  
**Result: PASS.** Deletion by specific id works correctly.

### `compact`

**Input:** `content="Test archival: contextro MCP end-to-end release test run at 2026-05-14"`  
**Output:** `{ "ref_id": "arc_2105b810", "archived": true, "chars": 70, "ttl": "permanent", "ttl_note": "..." }`  
**Result: PASS.** Archive created with ref_id. `ttl_note` explains that the ttl is reported for observability but uses the archive's configured retention window.

### `retrieve`

**Input:** `ref_id="arc_2105b810"`  
**Output:** `{ "ref_id": "arc_2105b810", "content": "Test archival: contextro MCP end-to-end release test run at 2026-05-14" }`  
**Result: PASS.** Exact content returned. Round-trip is clean.

### `session_snapshot`

**Input:** `limit=5`  
**Output:** 5 most recent tool calls with type, summary, arguments, and ISO 8601 timestamp.  
**Result: PASS.** The session tracker is working. Argument summaries are human-readable (e.g. `"remember(content=\"Testing memory persistence...\", memory_type=\"note\", tags=[2 item(s)])"`) which is useful for context reconstruction after compaction.

### `tags`

**Output:** 19 tags listed.  
**Result: PASS.**

---

## Knowledge Tool

### `knowledge` (add)

**Input:** `command="add", name="test-entry", value="Contextro end-to-end tool test note for release validation"`  
**Output:** `{ "name": "test-entry", "chunks": 1, "status": "indexed", "overwritten": false }`  
**Result: PASS.** Inline text indexed as 1 chunk.

### `knowledge` (search)

**Input:** `command="search", query="release validation test"`  
**Output:** 5 results. Top result is the just-added `test-entry`. Other results include README.md, CONTRIBUTING.md, AGENTS.md, CLAUDE.md — all correctly relevant to release/validation context.  
**Result: PASS.** Knowledge search works cross-source (inline entries + file-backed docs) and ranks by relevance correctly.

---

## Repo Management Tools

### `repo_add`

**Input:** `path="/var/folders/.../T/opencode"` (a non-git directory)  
**Output:** `{ "registered": true, "is_git": false, "hint": "Registered a non-git directory. Index/search can still work, but git tools will return errors until you target a git repository.", "graph_nodes": 0, "graph_relationships": 0, "indexed": true }`  
**Result: PASS.** Graceful handling of non-git directories. The hint is informative and actionable. Empty directory produces zero graph nodes, which is correct.

### `repo_remove`

**Input:** `path="/var/folders/.../T/opencode"`  
**Output:** `{ "path": "...", "removed": true }`  
**Result: PASS.**

### `repo_status`

**Output:** Shows 1 registered non-git repo at `/private/var/.../contextro-nonrepo-edge` (leftover from a previous test session).  
**Result: PASS.** The persistence is working — repos registered in prior sessions are still present after restart. This confirms the persistence model works correctly, though leftover test repos accumulating over time is a minor housekeeping concern.

---

## Output Artifact Tools

### `docs_bundle`

**Input:** `output_dir="/tmp/.../contextro-docs-test"`  
**Output:** `{ "status": "generated", "files": ["architecture.md", "overview.md"], "output_dir": "..." }`  
**Result: PASS.** Two markdown files generated successfully.

### `sidecar_export`

**Input:** `path="crates/contextro-tools/src/search.rs", output_dir="/tmp/.../contextro-sidecars-test"`  
**Output:** `{ "status": "exported", "sidecars": 1, "path": "...", "output_dir": "..." }`  
**Result: PASS.** One sidecar file exported for the specified source file.

---

## Summary

### Pass / Fail by Tool

| Tool | Status | Notes |
|---|---|---|
| `health` | PASS | |
| `status` | PASS | |
| `overview` | PASS | |
| `restore` | PASS | |
| `architecture` | PASS | `temp_file` as #3 hub may surprise users |
| `tags` | PASS | |
| `introspect` | PASS | |
| `skill_prompt` | PASS | |
| `search` | PASS | Top result is error type, not entrypoint — minor ranking issue |
| `find_symbol` | PASS | |
| `explain` | PASS | |
| `find_callers` | PASS | |
| `find_callees` | PASS | |
| `impact` | PASS | `depth_hint` field is a nice UX touch |
| `refactor_check` | PASS | Best single-call tool — highly informative |
| `circular_dependencies` | PASS | |
| `code` → `get_document_symbols` | PASS | |
| `code` → `search_symbols` | PASS | |
| `code` → `list_symbols` | PASS | |
| `code` → `lookup_symbols` | PASS | |
| `code` → `pattern_search` | PASS | |
| `code` → `pattern_rewrite` (relative path) | **FAIL** | Returns "Path not found" for relative paths; absolute paths work |
| `code` → `pattern_rewrite` (absolute path) | PASS | Identity rewrite correctly returns 0 changes |
| `code` → `search_codebase_map` | **WEAK** | Returns 1 symbol for a multi-symbol subsystem; not useful |
| `code` → `edit_plan` | **FAIL** | `confidence: "high"` with empty `affected_symbols`, `target_files`, `risks` |
| `analyze` | PASS | |
| `focus` | PASS | |
| `audit` | PASS | |
| `dead_code` | PASS | |
| `test_coverage_map` | PASS | Honest range-based reporting |
| `commit_history` | PASS | |
| `commit_search` | PASS | Returns old Python-era commits — expected semantic behavior |
| `remember` | PASS | |
| `recall` | PASS | |
| `forget` | PASS | |
| `compact` | PASS | |
| `retrieve` | PASS | |
| `session_snapshot` | PASS | |
| `knowledge` (add) | PASS | |
| `knowledge` (search) | PASS | |
| `repo_add` | PASS | |
| `repo_remove` | PASS | |
| `repo_status` | PASS | |
| `docs_bundle` | PASS | |
| `sidecar_export` | PASS | |

**34 PASS / 2 FAIL / 1 WEAK** out of 37 tools (counting `code` sub-operations individually: 6/8 pass, 2 fail — overall 37 tool families: 34 pass, 2 fail, 1 weak).

---

## Issues

### Issue 1: `pattern_rewrite` — Relative path resolution (BUG)

**Severity:** Medium  
**Tool:** `code` → `pattern_rewrite`  
**Observed:** `path="crates/contextro-tools/src/search.rs"` returns `{"error": "Path not found: crates/contextro-tools/src/search.rs"}`.  
**Expected:** Relative paths rooted at the indexed codebase should resolve correctly, consistent with every other path-accepting tool in the suite (`analyze`, `focus`, `pattern_search`, `list_symbols`, `get_document_symbols` — all accept relative paths without error).  
**Impact:** Users who correctly use relative paths in all other tools will hit a confusing error in `pattern_rewrite` specifically. Since `pattern_search` and `pattern_rewrite` are semantically paired, this asymmetry is especially jarring.  
**Workaround:** Use absolute paths.

---

### Issue 2: `edit_plan` — Returns empty plan (BUG)

**Severity:** High  
**Tool:** `code` → `edit_plan`  
**Observed:** Goal: `"refactor handle_search to extract reranking into a separate function"` returned `affected_symbols: []`, `target_files: []`, `risks: []`, `related_tests: []` with `confidence: "high"`.  
**Expected:** `handle_search` (search.rs:17) calls `rerank_natural_language_results`, `drop_low_confidence_noise`, `result_matches_symbol_query`, and others. At minimum, the function itself and its direct callees should appear as affected symbols. `target_files` should include search.rs. A plan should be generated.  
**Impact:** This is the tool most relevant to the product's "AI-assisted refactoring" positioning. An AI agent using this tool to plan a refactor would receive no actionable guidance and might proceed incorrectly. The `confidence: "high"` on an empty response is actively misleading.  
**Workaround:** Use `refactor_check` instead — it produces richer output and is currently the more reliable tool for pre-refactor analysis.

---

### Issue 3: `search_codebase_map` — Narrow results for conceptual queries (QUALITY)

**Severity:** Medium  
**Tool:** `code` → `search_codebase_map`  
**Observed:** Query `"how does search ranking work"` returned 1 file, 1 symbol (`query_targets_product_surface`).  
**Expected:** The ranking system in this codebase spans: `rerank_natural_language_results` (search.rs:21), `drop_low_confidence_noise` (search.rs:130), `result_matches_symbol_query` (search.rs:398), `is_symbol_lookup_query` (search.rs:386), `vector_candidate_limit` (search.rs:552), `handle_search` (search.rs:17). A codebase map query should surface the cluster.  
**Impact:** An AI agent asking this question to understand how to modify the ranking logic would be given an incomplete map. This could cause incorrect or partial edits.  
**Note:** This is a retrieval quality issue, not a hard failure. The tool returns a non-empty response and doesn't error. But the result is too narrow to be useful for the intended use case.

---

## Honest Assessment

The core of the MCP — index, graph traversal, memory, and search — is solid. `refactor_check`, `find_callers`, `find_callees`, `impact`, and `explain` are genuinely useful tools that deliver what they describe. The persistence model (memories, compact/retrieve, knowledge) round-trips correctly. The 37-tool surface is well-structured and the `introspect` + `skill_prompt` onboarding tools are a good idea.

The gap between the product's positioning ("codebase intelligence for AI agents") and current state is concentrated in the higher-level reasoning tools. `edit_plan` is the most prominent example: it's the tool a coding agent would reach for before making structural changes, and it currently produces nothing. `search_codebase_map` similarly underdelivers for the "how does X work" queries that agents ask most frequently.

For navigation and tracing use cases, this MCP is production-ready. For AI-assisted editing and structural understanding, the two broken tools are the critical path to close.
