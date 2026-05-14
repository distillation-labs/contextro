# Contextro v1.6.8 — Issues & Bugs

**Tested:** 2026-05-14 | **Version:** contextro@1.6.8 | **Score: 9/10**
**Method:** Deep adversarial testing — edge cases, empty inputs, boundary conditions, undocumented params

---

## What Got Fixed in v1.6.8

| Bug | Status |
|---|---|
| `sidecar_export output_dir=` param silently ignored | **FIXED** — output now goes to specified dir |

That's it. One fix. Everything else from v1.6.7 is unchanged.

---

## Bugs Still Present (carried from v1.6.7)

### HIGH — `knowledge add` accepts nonexistent file paths
`knowledge add name="fake" value="/nonexistent/path/fake.md"` returns `{"status":"indexed","chunks":1}`. No error. The string `/nonexistent/path/fake.md` is literally indexed as chunk content. Knowledge base is silently poisoned. Any agent relying on this has corrupt data with no signal.

### HIGH — `session_snapshot limit=` and `type=` params silently ignored
`session_snapshot limit=3` → returns all 20 events.
`session_snapshot type="search"` → returns all event types, no filtering.
Both params accepted without error, both completely ignored.

### HIGH — `commit_history since=` and `author=` params silently ignored
`commit_history since="2026-01-01"` → returns all commits.
`commit_history since="2027-01-01"` (future date) → still returns all commits. Definitive proof ignored.
`commit_history author="NOBODY_XYZNONEXISTENT"` → still returns all commits by Japneet Kalkat.
Both params accepted without error, both completely ignored.

### MEDIUM — `recall` requires non-empty query — no "list all memories" path
`recall query=""` → `{"error":"Missing required parameter: query"}`.
`recall tags=["architecture"] query=""` → same error even with tags filter.
No way to enumerate all memories. Workaround: `recall query="a"` as a catch-all hack.

### MEDIUM — `remember memory_type="bogustype"` silently coerced to "note"
Any invalid memory type is silently replaced with `"note"`. Response shows `memory_type: "note"` with no warning. Agents can't trust their specified type was honored.

### MEDIUM — `knowledge clear` command doesn't exist
`knowledge command="clear"` → `{"error":"Unknown knowledge command: clear"}`. No bulk-clear of knowledge base. Can only `remove` one doc by name at a time.

### MEDIUM — `knowledge search` always returns top-k on nonsense queries
`knowledge search query="xyznonexistent999"` → 5 results returned. No relevance threshold. Any query always returns top-k nearest neighbors regardless of how irrelevant. Agents can't detect "no match."

### MEDIUM — `compact ttl=` and `remember ttl=` — accepted but no observability
TTL params accepted without error. No `expires_at` in responses. No confirmation TTL was set. Zero visibility into when refs or memories will expire.

### MEDIUM — `architecture limit=` param silently ignored
`architecture limit=3` → always returns 10 hubs.
`architecture limit=100` → still 10 hubs.
Param not in schema, silently ignored.

### MEDIUM — `architecture` uses `degree` field, `analyze` uses `connections` — same metric, different names
`architecture` hub entries: `{degree, file, name}`.
`analyze` hub entries: `{connections, file, name}`.
Inconsistent field names for the same metric across two tools.

### MEDIUM — `dead_code limit=200` — returns 50 items with no explanation
`dead_code limit=200` → `total:200` but only 50 items returned, `truncated:true`. The hint says "use max_tokens for a different budget" but `max_tokens` isn't in the `dead_code` schema. `limit` controls the heuristic scan ceiling, not the output size — this distinction is never explained.

### MEDIUM — `dead_code include_public_api=true` and `include_tests=true` appear to be no-ops
Both params return identical results to the default. Accepted without error, may do nothing.

### LOW — `knowledge add` duplicate name has no overwrite signal
Adding the same `name` twice returns identical `{"status":"indexed"}` both times. No `"overwritten":true` flag. Silent upsert.

### LOW — `find_symbol` error never suggests `exact=false`
`find_symbol name="Browser"` → `"Symbol 'Browser' not found."` with no hint that `exact=false` exists for fuzzy/prefix matching. Agents hit a wall with no guidance.

### LOW — `find_callers`/`find_callees` `limit=` param silently ignored
`find_callers symbol="ainvoke" limit=5` → returns all 16 callers. Not in schema, silently ignored.

### LOW — `search limit=100` hard-capped at 50 with no signal
`search limit=100` → 50 results. No `truncated` flag. No `total` count. Silent downgrade.

### LOW — `forget memory_id="nonexistent"` is a silent no-op
Returns `{"deleted":0}` instead of an error. Callers can't distinguish bad ID from already-deleted.

### LOW — `analyze min_connections=` and `top_n=` params silently ignored
Not in schema. Both accepted without error, both ignored. Always returns top-10.

### LOW — `session_snapshot` and `sidecar_export` session-state dependent
`sidecar_export` only works when `index()` was called in the same MCP process session. Called in a fresh session → `sidecars:0`. No warning emitted.

### LOW — `health` response is minimal — no diagnostics
Only 3 fields: `indexed`, `status`, `uptime_seconds`. No server version, no memory usage, no vector store health, no error counts.

### LOW — `code search_symbols` param name is `symbol_name` not `query`
Sending `{"operation":"search_symbols","query":"watchdog"}` → `"Missing required parameter: symbol_name"`. Natural param name is `query` but actual is `symbol_name`. No hint in error.

### LOW — `code lookup_symbols symbols=[]` treated as missing
Empty list `[]` → `"Missing required parameter: symbols"`. Empty list is indistinguishable from absent param.

### LOW — `restore` doesn't prime symbol tools
`find_symbol` errors after `restore` until `index` is called again in the same process. `restore` loads graph stats but doesn't make symbol lookup work.

### INFO — `skill_prompt` documents happy path only
None of the known broken params (`session_snapshot limit/type`, `commit_history since/author`, `recall` empty query, `knowledge clear`, `compact ttl`) are mentioned. Agents using `skill_prompt` as guidance have no idea these don't work.

---

## What's Working Cleanly

- `health`, `status`, `index` (fast, accurate, incremental), `restore`
- `search` hybrid + bm25 (differentiated scores, relevant results, empty on no match)
- `find_symbol` exact + `exact=false` fuzzy
- `find_callers`, `find_callees`, `explain`, `focus` (file + directory)
- `overview`, `architecture` (hubs correct), `impact` (total field present), `analyze`, `audit`
- `dead_code` (basic), `circular_dependencies`, `test_coverage_map`
- `code` → all operations (get_document_symbols, search_symbols, pattern_search, lookup_symbols, search_codebase_map)
- `knowledge list`, `knowledge search`, `knowledge add`, `knowledge remove` (within-session + cross-session persistence)
- `remember`, `recall` (with query), `forget` (by `id` or `memory_id`)
- `repo_add`, `repo_status`, `repo_remove`
- `session_snapshot` (arguments included in events)
- `compact`, `retrieve` (persists across sessions, graceful not-found error)
- `commit_history`, `commit_search` (scores differentiated, case-insensitive, limit works)
- `docs_bundle`, `sidecar_export` (work when index is in-session)
- `skill_prompt`, `introspect` (tool names non-empty, scoped schema works)
- `impact max_depth=0` edge case handled
- `knowledge remove` by name works
- `repo_remove` by name works
