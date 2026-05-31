# Tool Decision Tree

Full routing guide for all 40 public Contextro tools plus the eight `code(...)`
operations. Use this reference when the short table in `SKILL.md` is not enough.

## Fast Default

1. `health({})` only if the server/runtime itself may be down.
2. `index({"path":"/repo"})` once per repo.
3. After `index()` returns `status: "done"`, start work. Use `status({})` only when
   active-repo or readiness details are still unclear.
4. Prefer `search`, `find_symbol`, `explain`, and `focus` before direct file reads.
5. Use `impact` or `refactor_check` before risky edits, and `completion_check` before
   claiming a rename or signature change updated all callers.
6. If you are unsure which tool or parameter shape fits, call
   `introspect({"query":"..."})` or `introspect({"tool":"..."})`.

## By Outcome

| Need | Best tool path | Notes |
|---|---|---|
| Index a new repo | `index({"path":"/repo"})` | Do not re-index before every query |
| Check runtime/server health | `health({})` | Use when the runtime itself may be unavailable |
| Check active repo and index state | `status({})` | Use when readiness or active scope is uncertain |
| Find code that does X | `search({"query":"X"})` | Default discovery tool |
| Find one exact definition | `find_symbol({"symbol_name":"ExactName"})` | Add `exact:false` for fuzzy lookup |
| Trace callers/callees | `find_callers(...)` / `find_callees(...)` | Call-graph exactness, not plain text |
| Understand one symbol | `explain({"symbol_name":"Symbol"})` | Start here before reading the full file |
| Check refactor blast radius | `refactor_check(...)` then `impact(...)` | `completion_check(...)` after the edit |
| Verify a rename/signature refactor is done | `completion_check({"claim":"all_callers_updated", ...})` | Use before claiming all callers were updated |
| Plan AST-aware or symbol-heavy edits | `code(...)` operations | See the dedicated section below |
| Compare or search another repo | `repo_add(...)` -> `search` / `explain` | `repo_add` registers, auto-indexes, and activates the repo; use `index(path)` later only for explicit re-indexing |
| Recover archives, memories, or session state | `retrieve`, `recall`, `restore`, `session_snapshot` | Pick the exact recovery path; see distinctions below |
| Generate packaged artifacts | `audit`, `docs_bundle`, `sidecar_export` | Use only when the user wants an artifact |
| Ask Contextro which tool fits | `introspect(...)` | Prefer this over guessing params or tool choice |

## Service Readiness And Repo Scope

| Tool | Use when | Prefer something else when | Example |
|---|---|---|---|
| `health` | You need to know whether the server/runtime is up | You only need active repo/index state | `health({})` |
| `status` | You need active repo, index counts, memory counts, or uptime | `index()` just returned `status: "done"` and work can start | `status({})` |
| `index` | A repo is new, changed, or not indexed yet | You are already on the active indexed repo | `index({"path":"/repo"})` |
| `repo_add` | You want to register, auto-index, and activate an external repo | You only need the current repo | `repo_add({"path":"/Users/alice/platform","name":"platform"})` |
| `repo_status` | You need the registered repo list | You need runtime stats or active repo (`status`) instead | `repo_status({})` |
| `repo_remove` | Temporary repo work is done and you want to remove it by name or path | You only need to switch queries inside the current repo | `repo_remove({"name":"platform"})` |

Notes:

- `repo_add` auto-indexes the repo and switches active scope. Run `index({"path":"..."})`
  later only when you explicitly want to re-index an already registered repo.
- `repo_remove` accepts either `name` or `path`.
- Do not simulate repo management by manually deleting files under storage.

## Discovery, Graph, And Orientation

| Tool | Use when | Prefer something else when | Example |
|---|---|---|---|
| `search` | You have a concept, symptom, identifier, or code phrase | You already know the exact symbol name | `search({"query":"authentication middleware"})` |
| `find_symbol` | You know the exact or near-exact symbol name | You need broad conceptual discovery | `find_symbol({"symbol_name":"BrowserSession","exact":true})` |
| `find_callers` | You need exact call-graph callers of a symbol | You need every text mention, including comments/docs | `find_callers({"symbol_name":"BrowserSession"})` |
| `find_callees` | You need exact outgoing call dependencies | You need a summary rather than raw callees | `find_callees({"symbol_name":"BrowserSession"})` |
| `explain` | You want a symbol summary with definition, callers, callees, and docs | You need the raw full file body | `explain({"symbol_name":"BrowserSession"})` |
| `focus` | You already narrowed to a file/dir and want a low-token context slice | You still need broad discovery | `focus({"path":"crates/contextro-tools/src/search.rs"})` |
| `overview` | You just opened an unfamiliar repo and need high-level totals and hotspots | You only need one symbol | `overview({})` |
| `architecture` | You want hub symbols, layers, and structure | You need file-level detail | `architecture({})` |
| `analyze` | You want scoped complexity/hotspot analysis for a path | You want whole-repo orientation first | `analyze({"path":"src/auth"})` |

Search mode selection:

| Query type | Mode | Example |
|---|---|---|
| Conceptual ("how does auth work") | `hybrid` (default) | `search({"query":"authentication flow"})` |
| Exact identifier | `bm25` | `search({"query":"CTX_STORAGE_DIR","mode":"bm25"})` |
| Semantic-only similarity | `vector` | `search({"query":"retry logic","mode":"vector"})` |

## Refactor Safety And Completion

| Tool | Use when | Prefer something else when | Example |
|---|---|---|---|
| `impact` | You need the explicit transitive blast radius before rename/delete/signature changes | You want a one-call summary first | `impact({"symbol_name":"BrowserSession","max_depth":3})` |
| `refactor_check` | You want definition + callers + callees + impact + risk in one call | You already know you only need the blast radius | `refactor_check({"symbol_name":"BrowserSession"})` |
| `completion_check` | You finished a rename/signature refactor and need to verify all callers were updated | You are still planning the change | `completion_check({"claim":"all_callers_updated","symbol_name":"BrowserSession","changed_files":["src/session.rs","src/main.rs"]})` |

Recommended refactor order:

```text
1. refactor_check(symbol_name="Symbol")
2. impact(symbol_name="Symbol") if the blast radius matters explicitly
3. explain(symbol_name="Symbol") or find_callers(symbol_name="Symbol") as needed
4. Make the edit
5. completion_check(claim="all_callers_updated", symbol_name="Symbol", changed_files=[...])
6. search(query="OldName", mode="bm25") for textual cleanup
```

## `code(...)` Operations

Use `code` when you need AST-aware symbol inventories, pattern search/rewrite, batch
symbol lookup, or a codebase map.

| Operation | Use when | Prefer something else when | Example |
|---|---|---|---|
| `get_document_symbols` | You need the symbols for one file | You need repo-wide discovery | `code({"operation":"get_document_symbols","path":"src/server.rs","include_signature":true})` |
| `search_symbols` | You want symbol-name discovery across the repo without hybrid semantic ranking | You want conceptual search or exact definition lookup | `code({"operation":"search_symbols","symbol_name":"auth"})` |
| `lookup_symbols` | You need batch exact symbol lookup for several names | One `find_symbol` is enough | `code({"operation":"lookup_symbols","symbols":["A","B","C"]})` |
| `list_symbols` | You want file symbols or a directory symbol inventory | You only need one exact symbol | `code({"operation":"list_symbols","path":"src"})` |
| `pattern_search` | You need AST-shaped structural search | Plain text or semantic search is enough | `code({"operation":"pattern_search","pattern":"fn $F($$$) -> Result","language":"rust"})` |
| `pattern_rewrite` | You need a structured rewrite with preview/apply flow | A normal text edit is cheaper | `code({"operation":"pattern_rewrite","pattern":"println!($MSG)","replacement":"tracing::info!($MSG)","language":"rust","dry_run":true})` |
| `edit_plan` | You want a heuristic plan for a symbol-centered change | You already know the edit steps | `code({"operation":"edit_plan","goal":"Replace legacy logging","symbol_name":"log_event"})` |
| `search_codebase_map` | You want a subsystem or architecture slice, not just one symbol | The task is narrow and symbol-specific | `code({"operation":"search_codebase_map","query":"indexing pipeline"})` |

Response-contract reminders:

- `get_document_symbols` and file-path `list_symbols` return columnar
  `{ file, columns, symbols, total }`.
- Directory-path `list_symbols` returns object rows with `callers` and `callees`.
- `lookup_symbols` omits `type` for unique exact matches unless `include_source=true`.
- Always run `pattern_rewrite` with `dry_run:true` first.

## Quality, Reporting, And Export Artifacts

| Tool | Use when | Prefer something else when | Example |
|---|---|---|---|
| `dead_code` | You want likely unreachable symbols, optionally scoped or filtered | You need a packaged report instead | `dead_code({"path":"src","limit":20})` |
| `circular_dependencies` | You want import-cycle groups | You need general architecture, not just cycles | `circular_dependencies({})` |
| `test_coverage_map` | You want static test-coverage bounds | You need runtime coverage numbers | `test_coverage_map({})` |
| `audit` | You want a packaged quality audit report | You only need one direct answer | `audit({})` |
| `docs_bundle` | You want generated documentation from the current index | You only need a summary in chat | `docs_bundle({"output_dir":".contextro-docs"})` |
| `sidecar_export` | You want `.graph.*` sidecars for files/dirs | You need human-facing docs instead | `sidecar_export({"path":"crates/contextro-tools/src"})` |

## Git, Memory, Knowledge, And Recovery

| Tool | Use when | Prefer something else when | Example |
|---|---|---|---|
| `commit_search` | Commit subjects are descriptive and you want semantic history search | Messages are terse or formulaic | `commit_search({"query":"payment flow refactor"})` |
| `commit_history` | You want recent commits in chronological order | You need semantic matching over commit messages | `commit_history({"limit":10})` |
| `remember` | You want to store a durable note, decision, preference, or status | You are archiving transient session state | `remember({"content":"Use CTX_STORAGE_DIR for RC runs","memory_type":"decision","tags":["release"]})` |
| `recall` | You want semantic retrieval of durable memories stored with `remember` | You have a `ref_id` from `compact` | `recall({"query":"release workflow","limit":3})` |
| `tags` | You want the known memory tags before filtering or forgetting | You need actual memory content | `tags({})` |
| `forget` | You want to delete memories by id, tag, or type | You want to archive current session context instead | `forget({"tags":"outdated"})` |
| `knowledge` | You want to index lightweight docs/notes in the active repo scope and search them later | You need cross-repo code discovery | `knowledge({"command":"search","query":"cache invalidation"})` |
| `compact` | You want to archive transient session context and get a `ref_id` | You want durable searchable memory instead | `compact({"content":"session summary"})` |
| `retrieve` | You have a `ref_id` returned by `compact` and want that archived content back | You need a project re-entry summary or durable memory search | `retrieve({"ref_id":"arc_ab12cd34"})` |
| `restore` | You want a synthesized project/session re-entry summary after restart or time away | You need the exact recent event log | `restore({})` |
| `session_snapshot` | You want recent raw session events or exact prior tool-call context | You want a narrative re-entry summary | `session_snapshot({"limit":10})` |

Knowledge commands worth remembering:

- `add`: add a named knowledge source from inline content or a file/directory path
- `search`: query indexed knowledge in the active repo scope
- `show`: inspect one source
- `list`: list indexed sources
- `update`: re-index a known source
- `remove` / `clear`: delete sources

## Self-Documentation And Bootstrap Helpers

| Tool | Use when | Prefer something else when | Example |
|---|---|---|---|
| `introspect` | You are unsure which Contextro tool fits, or you need exact parameter docs/examples | The tool choice is already obvious | `introspect({"tool":"completion_check"})` |
| `skill_prompt` | Another agent/client needs the bootstrap block for using Contextro well | You are doing normal repo discovery or analysis | `skill_prompt({})` |

## Important Distinctions

- `health` vs `status`: `health` is about runtime availability; `status` is about active
  repo/index/memory counts once the server is already up.
- `search` vs `find_symbol` vs `search_symbols` vs `lookup_symbols`:
  - `search` = concepts, symptoms, exact identifiers, general discovery
  - `find_symbol` = one exact or fuzzy definition lookup
  - `search_symbols` = symbol-name discovery across the repo
  - `lookup_symbols` = batch exact symbol lookup
- `impact` vs `refactor_check` vs `completion_check`:
  - `impact` = explicit transitive blast radius
  - `refactor_check` = one-shot pre-edit summary and risk
  - `completion_check` = post-edit caller completeness verification
- `retrieve` vs `restore` vs `session_snapshot` vs `recall`:
  - `retrieve` = archives created by `compact`
  - `restore` = synthesized project re-entry summary
  - `session_snapshot` = raw recent session events
  - `recall` = durable memories stored with `remember`
- `docs_bundle` vs `audit` vs `sidecar_export`:
  - `docs_bundle` = generated docs pack
  - `audit` = packaged quality report
  - `sidecar_export` = `.graph.*` artifacts for downstream tools

## When Not To Use Contextro

- Single-file edit where you already know the exact file and line -> read the file directly
- Reading `package.json`, `pyproject.toml`, or one known config file -> read the file directly
- Writing new code from scratch -> no Contextro needed unless repo discovery is part of the task
- Answering general programming questions -> no Contextro needed
