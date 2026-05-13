# Architectural Improvements — Deferred

These are improvements that require breaking changes, deeper design work, or multi-day implementation. They were identified during the v0.8→v1.6 development cycle and deferred because they couldn't be shipped incrementally without risking regressions.

---

## 1. Tool Consolidation (35 → ~15 tools)

**Intent:** Reduce schema token overhead by 50-70%. OpenAI and Anthropic both recommend <20 tools for optimal tool selection accuracy. Every tool definition costs 100-300 tokens in the system prompt.

**Why deferred:** Breaking API change. Existing agents have tool names hardcoded. Needs a migration path (accept old names as aliases for 2-3 versions).

**Proposed consolidation:**

| New tool | Replaces | Mechanism |
|----------|----------|-----------|
| `search` | `search`, `find_symbol`, `commit_search` | `mode` enum: `semantic`, `symbol`, `bm25`, `git_history` |
| `graph` | `find_callers`, `find_callees`, `impact`, `circular_dependencies` | `operation` enum |
| `analyze` | `analyze`, `dead_code`, `test_coverage_map`, `architecture` | `analysis` enum |
| `explain` | `explain`, `focus` | `depth` enum: `summary`, `standard`, `full` |
| `code` | Keep as-is (already consolidated) | — |
| `index` | `index`, `repo_add`, `repo_remove`, `repo_status` | `action` enum |
| `memory` | `remember`, `recall`, `forget`, `tags` | `action` enum |
| `session` | `compact`, `session_snapshot`, `restore`, `retrieve` | `action` enum |
| `knowledge` | Keep as-is | — |
| `overview` | `overview`, `audit`, `docs_bundle` | `scope` enum |
| `git` | `commit_search`, `commit_history` | `action` enum |
| `meta` | `status`, `health`, `introspect`, `skill_prompt` | `action` enum |
| `export` | `sidecar_export` | — |
| `refactor_check` | Keep as-is (already composite) | — |

**Approach:**
1. Add the consolidated tools alongside the old ones (both work)
2. Mark old tools as deprecated in `introspect` responses
3. After 2 versions, remove old tools and bump major version

**Effort:** 3-5 days. Mostly mechanical dispatch changes + schema updates.

---

## 2. Streaming Partial Results

**Intent:** Reduce perceived latency to <1ms. BM25 results arrive in <0.1ms but vector search takes 5-50ms. Streaming BM25 first gives the agent something to work with immediately.

**Why deferred:** The `rmcp` crate's `ServerHandler` trait returns a `Future<Output = Result<CallToolResult>>` — a single response. Streaming requires using `notifications/progress` during tool execution, which means the handler needs access to the notification channel.

**How to approach:**
1. In `call_tool`, before awaiting the result, spawn the tool execution on a background task
2. Use `rmcp`'s notification mechanism to emit `notifications/progress` with partial results
3. The final `CallToolResult` contains the complete fused result
4. For `search`: emit BM25 results immediately, then vector results, then fused

**MCP spec support:** The 2024-11-05 spec defines `notifications/progress` with `progressToken`. The Streamable HTTP transport supports SSE for streaming. stdio transport multiplexes on stdout.

**Effort:** 2-3 days. Requires understanding rmcp's internal notification API.

---

## 3. Scope-Based Call Resolution

**Intent:** Eliminate false edges from name collisions. Currently if two files both define `validate`, any call to `validate` gets attributed to whichever was indexed first.

**Why deferred:** Requires parsing import statements and building a per-file import map. TypeScript's module resolution (barrel files, path aliases, `tsconfig.json` paths) is genuinely complex.

**How to approach:**
1. During parsing, extract import statements into a per-file map: `{ "validate": "src/lib/validators.ts" }`
2. Store the import map alongside the file hashes in `project_storage_dir`
3. In `build_graph`, when resolving a call `validate` from file A, look up A's import map to find the target file, then find the node in that file
4. Fall back to name-based resolution when the import map doesn't have an entry

**Complexity:** The hard part is barrel files (`export * from './validators'`). Resolving these requires recursively following re-exports. TypeScript's own language server takes seconds for this on large repos. A pragmatic approach: resolve direct imports only, skip barrel files.

**Effort:** 5-7 days for direct imports. Barrel file resolution adds another 3-5 days.

---

## 4. Persistent Graph + Vector Index

**Intent:** Eliminate re-indexing entirely on server restart. Currently the BM25 index is persistent but the graph and vector index are rebuilt from scratch.

**Why deferred:** The graph is an in-memory `HashMap` structure. Persisting it requires serialization (serde to disk) or a proper graph database. The vector index uses a simple `Vec<(Vec<f32>, SearchResult)>` — persisting it requires a proper vector store (LanceDB is already in deps but unused).

**How to approach:**
1. **Graph:** Serialize `GraphInner` to a binary format (bincode or MessagePack) at `project_storage_dir/graph.bin`. Load on startup if exists.
2. **Vector index:** Use LanceDB (already in Cargo.toml) to store embeddings on disk. Replace the in-memory `VectorIndex` with a LanceDB table.
3. **Startup flow:** Check if `graph.bin` + LanceDB table + BM25 index all exist → load them → set `indexed = true` → server is ready immediately without `index()` call.

**Effort:** 3-4 days for graph persistence. 5-7 days for LanceDB integration.

---

## 5. Incremental Graph Update (True Partial Re-indexing)

**Intent:** When 3 files change out of 8,000, only re-parse those 3 files and update their graph edges. Currently we detect which files changed but still do a full re-parse of all files.

**Why deferred:** The `IndexingPipeline::index()` method takes a directory path and always discovers + parses all files. Splitting it into "parse these specific files" requires refactoring the pipeline API.

**How to approach:**
1. Add `IndexingPipeline::index_files(paths: &[PathBuf])` that only parses the given files
2. In `handle_index`, when `is_incremental && changed_count > 0`:
   - Remove old graph nodes for changed/deleted files (`graph.remove_file_nodes`)
   - Parse only added + modified files
   - Add new nodes and edges
   - Update BM25: delete old docs for changed files, insert new chunks
   - Recompute PageRank
3. This turns a 650ms full re-index into a ~50ms partial update for typical edit sessions

**Effort:** 2-3 days. The infrastructure (hash tracking, file removal) already exists.

---

## 6. Markdown Response Format

**Intent:** 15-30% token reduction. Markdown uses fewer structural tokens than JSON (no quotes, braces, commas for key-value pairs). LLMs read Markdown more naturally than JSON.

**Why deferred:** Requires a dual-format architecture. The `content` field (what the LLM reads) should be Markdown, while `structuredContent` (for programmatic clients) stays JSON. This means every tool needs two renderers.

**How to approach:**
1. Create a `ResponseRenderer` trait with `render_markdown()` and `render_json()` methods
2. In `dispatch()`, render the tool output as Markdown for the `content` field
3. Keep the JSON in a `structuredContent` field (MCP spec supports this)
4. Example Markdown for search:
   ```
   ## search results (3)
   
   - **validate_token** `src/auth/middleware.rs:42` score=0.95
   - **handle_login** `src/api/routes.rs:18` score=0.87
   - **refresh_session** `src/auth/session.rs:91` score=0.72
   ```

**Effort:** 5-7 days. Every tool handler needs a Markdown formatter.

---

## 7. Progressive Tool Discovery

**Intent:** For agents using all 37 tools, schema overhead is ~6000 tokens per request. Progressive discovery reduces this to ~300 tokens initially.

**Why deferred:** Requires agents to adopt a two-step pattern (discover → use) which not all MCP clients support well.

**How to approach:**
1. When `CTX_DISCOVERY=progressive`, expose only 3 meta-tools:
   - `discover(category?)` → list tool categories and descriptions
   - `describe(tool)` → full schema for one tool
   - `execute(tool, params)` → run any tool by name
2. Categories: `search`, `graph`, `analysis`, `memory`, `git`, `admin`
3. The agent calls `discover()` once, then `describe()` only for tools it needs

**Effort:** 2-3 days. Mostly dispatch routing changes.

---

## Priority Order

If I had to pick the order to implement these:

1. **#5 (True partial re-indexing)** — highest user impact, lowest risk, 2-3 days
2. **#4 (Persistent graph + vector)** — eliminates the need for `index()` on restart, 3-4 days
3. **#1 (Tool consolidation)** — biggest token reduction, but breaking change, 3-5 days
4. **#6 (Markdown responses)** — 15-30% token reduction, 5-7 days
5. **#3 (Scope-based resolution)** — improves accuracy, 5-7 days
6. **#2 (Streaming)** — UX improvement, 2-3 days
7. **#7 (Progressive discovery)** — niche optimization, 2-3 days

Total: ~25-35 days of focused work to implement all 7.
