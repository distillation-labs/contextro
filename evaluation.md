# Contextro MCP Evaluation

## Scope

This document evaluates the local **Contextro MCP** server against the `bozeman` repo:

- **Repo:** `/Users/japneetkalkat/conductor/workspaces/platform/bozeman`
- **Server:** `Contextro 3.2.4`
- **Protocol:** `2025-11-25`
- **Transport:** `streamable-http` on `http://127.0.0.1:8123/mcp`
- **Benchmark client:** official Python `mcp` client over real MCP transport

Raw benchmark output was captured to:

- `/Users/japneetkalkat/.copilot/session-state/b532e0fe-58dd-4d38-adfc-70741e56db72/files/contextro_benchmark_results.json`

## Methodology

1. Started an isolated HTTP Contextro server.
2. Measured **wall-clock latency** with `perf_counter()` for MCP `initialize`, `ping`, `list_tools`, and Contextro tool calls.
3. Benchmarked both:
   - **cold server state** before `index(path)`
   - **indexed state** after `index(path)`
4. Ran **2-3 repetitions** for fast/idempotent tools and **1 run** for heavy or mutating tools.
5. Used real repo paths and representative queries/symbols.
6. Per-tool evaluation included:
   - latency
   - output shape
   - token/sandbox behavior when available
   - qualitative correctness notes

## Executive summary

**Overall verdict:** Contextro MCP is **fast and useful for indexed symbol lookup, focused code context, impact analysis, search, and lightweight repo introspection**. It is also **token-aware** and uses sandboxing aggressively for large outputs.  
It is **not yet reliable as a source of truth for repo-wide dead-code, coverage, maintainability, or some Next.js/App Router reachability claims**.

### What is working well

- MCP transport overhead is low:
  - `initialize`: **3.09 ms**
  - `ping`: **2.94 ms**
  - `list_tools`: **4.06 ms**
- Indexed navigation tools are quick:
  - `find_symbol`: ~**4.8 ms**
  - `find_callees`: ~**4.72 ms**
  - `impact`: ~**8.39 ms**
  - `focus`: ~**8.98 ms**
  - `code_search_symbols`: ~**4.69 ms**
- Search is token-disciplined:
  - repeated search query returned **271 tokens**
  - `budget_applied: true`
  - `adaptive_applied: true`
  - `sandbox_ref` returned for overflow
- Large reports are sandboxed instead of flooding the client:
  - `impact`: **1676 full_tokens**
  - `test_coverage_map`: **20956 full_tokens**
  - `dead_code`: **32525 full_tokens**
  - `audit`: **55416 full_tokens**
- Artifact tools worked cleanly:
  - `docs_bundle`
  - `sidecar_export` generate + clean
  - `skill_prompt`
  - `compact`
  - `remember` / `recall` / `forget`
  - `knowledge add/search/remove`

### What is not good

- A fresh HTTP server starts **healthy but unindexed** even when on-disk data already exists:
  - `health_cold.indexed = false`
  - `status_cold.codebase_path = null`
  - most useful tools are effectively unavailable until `index(path)` is called
- Repo-wide static-analysis outputs are not trustworthy enough in this codebase:
  - `dead_code()` flags live-looking Next.js auth routes as unreachable
  - `test_coverage_map()` reports extremely low static coverage that likely mixes config/runtime reachability limitations with real gaps
  - `analyze()` and `audit()` report **maintainability_index = 0**
  - `complexity.average_complexity = 0.0` across **7923 functions**
- Symbol disambiguation can be wrong:
  - `find_symbol(getUserId)` resolves `convex/lib/authIdentity.ts`
  - but its listed callees point to helpers from another `getUserId` implementation in `packages/convex-utils/permissions.ts`
- Some tool contracts are stricter or less ergonomic than expected:
  - `code.pattern_rewrite` requires `file_path`; `path` is not enough
  - `knowledge update` requires `path`, so inline text contexts are not easily updated by `context_id` alone
- File-count reporting is inconsistent across tools:
  - `index.total_files = 8520`
  - `status.graph.total_files = 4485`
  - `overview.total_files = 4485`
  - `architecture.total_files = 4485`
  - `test_coverage_map.summary.test_files + production_files = 8520`

## MCP transport performance

| Operation | Result |
| --- | ---: |
| initialize | 3.09 ms |
| ping | 2.94 ms |
| list_tools | 4.06 ms |
| tools exposed | 35 |

**Assessment:** the MCP layer itself is not the bottleneck. Most latency comes from tool logic, indexing state, or large-response assembly.

## Cold-start behavior

| Operation | Median | Result |
| --- | ---: | --- |
| health_cold | 2.56 ms | Healthy server, but all engines reported `false` and `indexed=false` |
| status_cold | 2.38 ms | `codebase_path=null`, hint says to run `index` |
| index_initial_load | 4258.76 ms | Wall-clock for making the server usable |

`index_initial_load` returned internal `time_seconds = 0.9706`, but end-to-end wall time was **4258.76 ms**. That gap suggests extra overhead beyond the internal indexing timer: transport, setup/warm-up, persistence loading, or server-side post-processing.

**Operational conclusion:** this MCP is fast **after binding to a repo**, but not immediately useful on a fresh HTTP server until `index(path)` runs.

## Selected benchmark results

| Benchmark | Median | Notes |
| --- | ---: | --- |
| status_indexed | 2.71 ms | Indexed state available; graph + branch + chunks |
| overview | 113.95 ms | Good compressed repo summary |
| architecture | 525.04 ms | Useful layered summary; noticeably slower but acceptable |
| analyze | 30.71 ms | Fast, but quality metrics look suspect |
| search_low_budget | 133.84 ms | 271 tokens, sandboxed, adaptive trimming |
| search_high_budget | 97.07 ms | Same 271 tokens and same results as low-budget run |
| find_symbol_requirePageAccess | 4.80 ms | Strong result quality |
| impact_requirePageAccess | 8.39 ms | 1676 full_tokens, sandboxed |
| commit_search | 12.36 ms | Good balance of speed and usefulness |
| docs_bundle | 136.82 ms | Produced 4 docs |
| sidecar_export_generate | 9.75 ms | Generated 1 sidecar |
| audit | 10776.50 ms | Heavy but sandboxed; highest cost among analysis tools |
| dead_code | 9.49 ms | Fast, but correctness is questionable |
| test_coverage_map | 9.85 ms | Fast, but likely overstates uncovered surface |
| index_incremental | 1454.41 ms | Internal `time_seconds = 1.3564` |

## Token-efficiency evaluation

### Good

- `search` is clearly designed for token control:
  - trimmed output
  - adaptive limiting
  - budget flags
  - sandbox references for overflow
- `impact`, `audit`, `dead_code`, and `test_coverage_map` all avoid returning full payloads inline.
- `focus` gives a compact file slice with a compressed preview instead of dumping the whole file.
- `session_snapshot` is very small and cheap.

### Not good

- `retrieve` is dangerous if called casually:
  - one sandbox retrieval returned **267,738 characters** in **15.47 ms**
  - efficient on the server, but potentially terrible for downstream token usage
- Increasing `context_budget` from **400** to **1600** for the tested search query produced **the same 271-token output**, so the larger budget did not improve recall or detail in that case.

### Cache behavior

Post-benchmark `status` showed:

```json
"cache": {
  "hits": 2,
  "misses": 4,
  "hit_rate": 0.333,
  "size": 0
}
```

So there is some observable reuse, but not strong enough yet to call caching a major performance win in this session.

## Correctness and quality findings

### Strong results

- `find_symbol(requirePageAccess)` returned the correct file, line range, and caller counts.
- `impact(requirePageAccess)` produced a plausible blast radius and sandboxed the larger result.
- `focus(auth callback route)` returned useful symbols, calls, preview, and blast-radius framing.
- `commit_search` and `commit_history` both returned plausible history quickly.
- `docs_bundle`, `sidecar_export`, `skill_prompt`, `remember/recall/forget`, and `knowledge add/search/remove` all behaved as expected.

### Weak or incorrect results

1. **Next.js route reachability appears under-modeled**
   - `dead_code()` reports:
     - `apps/app/src/app/[locale]/(auth-shell)/auth/callback/page.tsx`
     - `apps/app/src/app/[locale]/(auth-shell)/auth/complete/page.tsx`
     - `apps/app/src/app/[locale]/(auth-shell)/auth/login/page.tsx`
   - as unreachable from production entry points.
   - That is a major correctness warning for App Router codebases.

2. **Import tracking in `focus()` is incomplete**
   - `focus_auth_callback.imports = []`
   - but the preview clearly shows imports from `@/lib/auth/clerk-domain` and `@/features/auth/utils/redirect-url`.

3. **Duplicate-symbol disambiguation is wrong**
   - `find_symbol(getUserId)` returns `convex/lib/authIdentity.ts`
   - but `top_callees` and `find_callees(getUserId)` include:
     - `hasOrgClaim`
     - `isRecord`
   - which belong to a different `getUserId` implementation.

4. **Static quality metrics are not credible yet**
   - `maintainability_index = 0`
   - `average_complexity = 0.0`
   - `max_complexity = 0`
   - despite a large TypeScript repo and obvious non-trivial functions

5. **Count semantics are inconsistent**
   - Some tools report parsed graph files (**4485**)
   - some report discovered files (**8520**)
   - the tool outputs do not make that distinction explicit enough

## Tool-contract and UX issues

| Operation | Issue |
| --- | --- |
| `code.pattern_rewrite` | Returned `file_path required for pattern_rewrite`; `path` alone is insufficient |
| `knowledge update` | Returned `path required for 'update'`; inline contexts are awkward to update |
| fresh `health` / `status` | Server looks healthy but is unusable until `index(path)` |
| `repo_status` after `repo_add(index_now=false)` | Fast, but not very informative because totals remain zero until that repo is indexed |

## Full benchmark inventory

| Label | Tool | Runs | Median | Notes |
| --- | --- | ---: | ---: | --- |
| health_cold | health | 3 | 2.56 ms | |
| status_cold | status | 3 | 2.38 ms | |
| index_initial_load | index | 1 | 4258.76 ms | `time_seconds=0.9706`, files=8520, symbols=14227 |
| health_indexed | health | 3 | 2.71 ms | |
| status_indexed | status | 3 | 2.71 ms | |
| overview | overview | 2 | 113.95 ms | |
| architecture | architecture | 1 | 525.04 ms | |
| analyze | analyze | 1 | 30.71 ms | |
| audit | audit | 1 | 10776.50 ms | `full_tokens=55416`, sandboxed |
| circular_dependencies | circular_dependencies | 1 | 7.58 ms | |
| dead_code | dead_code | 1 | 9.49 ms | `full_tokens=32525`, sandboxed |
| test_coverage_map | test_coverage_map | 1 | 9.85 ms | `full_tokens=20956`, sandboxed |
| restore | restore | 2 | 83.19 ms | |
| session_snapshot | session_snapshot | 2 | 5.48 ms | |
| introspect | introspect | 2 | 4.65 ms | |
| skill_prompt_write | skill_prompt | 1 | 5.40 ms | no-op write, `changed=false` |
| docs_bundle | docs_bundle | 1 | 136.82 ms | produced 4 docs |
| commit_history | commit_history | 1 | 144.73 ms | |
| commit_search | commit_search | 2 | 12.36 ms | |
| search_low_budget | search | 2 | 133.84 ms | 271 tokens, sandboxed, adaptive |
| search_high_budget | search | 2 | 97.07 ms | same 271-token output as low budget |
| code_generate_codebase_overview | code | 1 | 8.40 ms | |
| code_search_symbols | code | 2 | 4.69 ms | |
| code_lookup_symbols | code | 2 | 3.99 ms | |
| code_get_document_symbols | code | 2 | 19.23 ms | |
| code_search_codebase_map | code | 1 | 6.16 ms | |
| code_pattern_search | code | 1 | 63.11 ms | valid, zero matches |
| code_pattern_rewrite_dry_run | code | 1 | 4.65 ms | error: `file_path required for pattern_rewrite` |
| find_symbol_requirePageAccess | find_symbol | 2 | 4.80 ms | good result |
| find_symbol_getUserId | find_symbol | 2 | 4.80 ms | symbol/callee conflation issue |
| find_callers_requirePageAccess | find_callers | 2 | 8.06 ms | |
| find_callees_getUserId | find_callees | 2 | 4.72 ms | mixed callees from duplicate symbol |
| explain_requirePageAccess | explain | 2 | 27.16 ms | |
| impact_requirePageAccess | impact | 2 | 8.39 ms | `full_tokens=1676`, sandboxed |
| focus_auth_callback | focus | 2 | 8.98 ms | useful preview, incomplete imports |
| index_incremental | index | 1 | 1454.41 ms | `time_seconds=1.3564`, files=8520 |
| compact_archive | compact | 1 | 5.86 ms | |
| remember_temp | remember | 1 | 9.16 ms | stored session memory |
| recall_temp | recall | 2 | 16.79 ms | |
| repo_add_temp | repo_add | 1 | 63.27 ms | registered temp repo only |
| repo_status | repo_status | 2 | 4.75 ms | registered repo totals stayed zero |
| repo_remove_temp | repo_remove | 1 | 3.65 ms | |
| sidecar_export_generate | sidecar_export | 1 | 9.75 ms | generated 1 sidecar |
| sidecar_export_clean | sidecar_export | 1 | 4.42 ms | removed 1 sidecar |
| knowledge_show_before | knowledge | 1 | 3.95 ms | |
| knowledge_add_temp | knowledge | 1 | 8.39 ms | indexed temp context |
| knowledge_search_temp | knowledge | 2 | 22.02 ms | |
| retrieve_first_sandbox | retrieve | 1 | 15.47 ms | returned 267,738 chars |
| forget_temp | forget | 1 | 7.64 ms | deleted temp memory |
| knowledge_update_temp | knowledge | 1 | 5.40 ms | error: `path required for 'update'` |
| knowledge_remove_temp | knowledge | 1 | 11.81 ms | removed temp context |
| knowledge_show_after | knowledge | 1 | 7.08 ms | |

## Recommendations

### High priority

1. **Auto-restore last indexed repo on HTTP server startup** or expose a startup flag that binds the server to a known codebase immediately.
2. **Fix Next.js/App Router entry-point modeling** before relying on `dead_code`, `focus.entry_point`, or reachability-derived coverage in this repo type.
3. **Fix duplicate-symbol disambiguation** so `find_symbol`, `find_callees`, and derived summaries stay on the same definition.
4. **Clarify file-count semantics** across `index`, `status`, `overview`, `architecture`, and `test_coverage_map`.

### Medium priority

1. Make `knowledge update` work for inline contexts by `context_id`.
2. Accept `path` consistently for `code.pattern_rewrite`, or make the error/help text more explicit.
3. Improve `focus()` import extraction for alias-heavy TypeScript projects.
4. Surface stronger cache diagnostics if search caching is meant to be a user-facing performance feature.

### Current usage guidance

Use Contextro confidently for:

- symbol lookup
- caller/callee tracing
- impact analysis
- focused file context
- compact repo summaries
- commit search/history
- token-aware search

Treat these as **advisory only**, not truth sources:

- dead code
- static coverage
- maintainability/complexity metrics
- App Router reachability
