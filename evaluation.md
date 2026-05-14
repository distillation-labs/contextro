# Contextro v1.6.5 — Issues & Bugs

**Tested:** 2026-05-14 | **Version:** contextro@1.6.5 | **Score: 9/10**

---

## What Got Fixed in This Version

| Bug | Status |
|---|---|
| `knowledge add` vectors not persisted across sessions (P0) | **FIXED** |
| Auto-indexed docs invisible in `knowledge list` | **FIXED** |
| Auto-indexed docs not searchable | **FIXED** |
| `impact` missing `total` field in response | **FIXED** |
| `dead_code` had no filter params | **FIXED** — `include_public_api`, `include_tests`, `exclude_paths`, `limit` now exist |

---

## Remaining Issues

### P1 — `introspect` (no args) regression
`introspect()` returns 37 tool entries but all with empty `name` fields — tool names are stripped from the response. Was working in v1.6.4.

```json
{"tools": [{"name": "", "description": "..."}, ...]}
```

Anything that relies on `introspect()` to discover tool names is broken.

### P1 — `commit_search` returns 0 results
Queries like `"fix"` and `"knowledge"` return `{"commits":[],"total":0}`. `commit_history` works fine — the search index is broken or not covering the commit history. Regressed from v1.6.4 where at least some results were returned.

### P2 — `forget` param mismatch with `remember`
`remember` returns `{"id": "mem_..."}` in the response.
`forget` requires `{"memory_id": "mem_..."}` as input.

Passing the `id` field directly from `remember` into `forget` produces:
```
{"error": "Provide memory_id, tags, or memory_type to forget"}
```

Any agent pipeline that does `forget(id=remember_response.id)` will fail. Either `forget` should accept `id` as an alias, or `remember` should return `memory_id`.

### P3 — `knowledge search "milestones"` → 0 results
Minor vocabulary miss — ROADMAP.md doesn't use the literal word "milestones" so the search returns nothing. Low-severity since it's a content issue not a bug, but worth noting for sparse/domain-specific docs.

---

## What's Working Fine

Everything else passes cleanly:

- `health`, `status`, `index`, `restore` — fast, accurate
- `search` hybrid + bm25 — differentiated scores, relevant results
- `find_symbol`, `find_callers`, `find_callees`, `explain`, `focus` — accurate, explicit errors on bad paths
- `overview`, `architecture`, `impact` (now with `total` field), `analyze`, `audit` — correct
- `dead_code` — 50 symbols, filter params now available
- `circular_dependencies`, `test_coverage_map` — correct
- `code` → all operations (get_document_symbols, search_symbols, pattern_search, lookup_symbols, search_codebase_map)
- `knowledge list` — now shows auto-indexed AND manually added docs
- `knowledge search` — auto-indexed docs searchable, manually added docs persist across sessions
- `knowledge add` — persists to disk, survives re-index
- `remember`, `recall` — cross-session persistent, clean misses
- `repo_add`, `repo_status`, `repo_remove` — all working
- `session_snapshot` — arguments included in all events
- `compact`, `retrieve` — persists across sessions
- `docs_bundle`, `sidecar_export` — correct output
- `commit_history` — correct
- `introspect tool="X"` — scoped schema works correctly
- `skill_prompt` — clean bootstrap block
