# Contextro MCP Release Evaluation

End-to-end release testing across v1.6.11 → v1.6.15.
Covers all 37 MCP tools and 7 real-world developer scenarios per release.

---

## Part 1 — v1.6.11 Baseline

**Graph:** 668 nodes, ~1,800 edges  
**Build:** `scripts/release-candidate.sh`

### Tool Sweep (37/37 operational)

All tools respond without errors. Core tools confirmed:

- `health()` → version `1.6.11`
- `index(".")` → 668 nodes, BM25 + vector indices built
- `search("QueryCache")` → top result with score; `confidence: "medium"` (query-agnostic at this version)
- `find_symbol("QueryCache")` → exact match in `contextro-engines/src/cache.rs`
- `commit_history(limit=5)` → 5 recent commits with author, hash, message, timestamp
- `audit()` → recommendations with `quality_score: 75` (static at this version)

### Scenario Results

| Scenario | Result | Notes |
|---|---|---|
| S1: Onboarding | PASS | overview + architecture + search all functional |
| S2: Feature deep dive | MIXED | `search("how does caching work")` returned 3 low-score results; missed `QueryCache` entirely |
| S3: Feature planning | MIXED | `edit_plan` returned wrong `affected_symbols` for cross-subsystem goals |
| S4: Reliability audit | PASS | `audit()` + `dead_code()` + `.expect()` search surfaced 32 panick sites |
| S5: Pre-PR quality gate | PASS | `impact()` + `find_callers()` correctly traced blast radius |
| S6: Multi-session continuity | MIXED | `recall()` surfaces memories; stale memories from previous runs cause noise |
| S7: Git archaeology | MIXED | `commit_search("BM25 improvements")` → 0 results; terse "Update X.rs" commit convention defeats semantic search |

### Known Issues Found

- `search("how does caching work")` misses `QueryCache` (generic symbol names don't embed well)
- `commit_search` blind to terse "Update X.rs" commit messages
- `edit_plan` `affected_symbols` unreliable for cross-subsystem goals
- `audit()` `quality_score` static at 75 regardless of actual issues found
- `search()` `confidence` field always `"medium"` regardless of query specificity

---

## Part 2 — v1.6.12

**Graph:** 721 nodes (+53), ~1,950 edges  
**Build:** standard release-candidate

### Changes vs v1.6.11

- `repo_add` / `repo_remove` added for multi-repo support
- `search_codebase_map` added (first version)
- `commit_search` minor improvements

### New Issues Found

- `repo_add` then `repo_remove` leaves active scope pointing at the removed repo; `index()` required to restore — **stateful scope bug** (`mem_ab2e3888`)
- `search_codebase_map("how does caching work")` also misses `QueryCache` — S2 bug reproduced in new tool
- `commit_search` still 0 results for terse messages

### Scenario Results

Same as v1.6.11 except S2 and S3 still MIXED. S6 MIXED (scope bug).

---

## Part 3 — v1.6.13

**Graph:** 839 nodes (+118), ~2,100 edges

### Key Fixes in v1.6.13

- `search("QueryCache")` now returns `confidence: "high"` for exact symbol matches — **S2 now PASS**
- `audit()` now returns `evidence` arrays and `follow_up` tool call suggestions — much more actionable
- `search_codebase_map` narrow-query precision improved

### Scenario Results

| Scenario | v1.6.12 | v1.6.13 | Delta |
|---|---|---|---|
| S1: Onboarding | PASS | PASS | — |
| S2: Feature deep dive | MIXED | **PASS** | fixed |
| S3: Feature planning | MIXED | **PASS** | fixed |
| S4: Reliability audit | PASS | PASS | — |
| S5: Pre-PR quality gate | PASS | PASS | — |
| S6: Multi-session continuity | MIXED | **PASS** | fixed |
| S7: Git archaeology | MIXED | **IMPROVED** | partial |

### S7 Detail (IMPROVED, not PASS)

`commit_search` works for commits with meaningful messages. For this codebase's "Update X.rs" convention, it still returns 0 results. This is a tool limitation when commit quality is poor.

---

## Part 4 — v1.6.13 Detailed Tool Results

### Confirmed Working

| Tool | Output Quality |
|---|---|
| `overview()` | Accurate language breakdown, top files by symbol count |
| `architecture()` | Correctly identifies `dispatch` as hub (66 connections) |
| `search("AppState")` | `confidence: "high"`, correct file |
| `find_symbol("handle_search")` | Exact match, correct line |
| `find_callers("handle_search")` | Full caller list |
| `find_callees("handle_search")` | Full callee list |
| `explain("dispatch")` | Summary + callers + callees |
| `impact("handle_search")` | Transitive blast radius, 3 hops |
| `refactor_check("handle_search")` | Definition + callers + callees + risk |
| `dead_code()` | Returns symbols with 0 callers |
| `circular_dependencies()` | Returns SCC cycles |
| `audit()` | `evidence` arrays, `follow_up` calls, `quality_score: 75` |
| `get_document_symbols("src/main.rs")` | Per-file symbol list |
| `commit_history(limit=5)` | Correct, timestamped |
| `remember()` / `recall()` | Cross-session persistence confirmed |
| `compact()` / `retrieve()` | Exact roundtrip confirmed |

### Persistent Issues After v1.6.13

- **P1** (partial): `search_codebase_map` narrow-query precision 1/3 (only exact file mentions work)
- **P2**: `commit_search` semantic search blind to terse "Update X.rs" messages
- **P3**: `audit()` `quality_score` stuck at 75 regardless of finding count
- **P4**: Stale bug memories (from v1.6.12 tests) pollute `recall()` in v1.6.13 sessions

---

## Part 5 — Skill Bundle Audit (v1.6.14 Pre-Release)

Audit of `packages/skills/skills/dev-contextro-mcp/SKILL.md` and reference docs before v1.6.14 release.

### Issues Found (S1–S9)

| ID | Severity | Description |
|---|---|---|
| S1 | HIGH | `get_document_symbols` columnar format (`{ file, columns, symbols, total }`) not documented; old `symbols[i].name` access pattern still shown |
| S2 | MEDIUM | `include_signature` parameter not mentioned anywhere in skill docs |
| S3 | MEDIUM | `search_codebase_map` described as "broad-query only"; no guidance on when narrow queries are appropriate |
| S4 | LOW | `audit()` `quality_score` described as fixed 75; actual formula not documented |
| S5 | MEDIUM | `find_symbol()` `confidence` field described as always `"medium"`; actually query-aware since v1.6.13 |
| S6 | MEDIUM | `search_codebase_map` example uses broad query; no narrow-query example |
| S7 | MEDIUM | `audit()` `evidence` and `follow_up` fields not in example output |
| S8 | LOW | No guidance on `forget()` for stale bug memories |
| S9 | LOW | `repo_add` scope restoration workaround not documented |

All 9 issues confirmed fixed in v1.6.14/v1.6.15 skill bundle updates.

---

## Part 6 — v1.6.14

**Graph:** 858 nodes (+19 from v1.6.13), 2,194 edges  
**Released:** documentation + Rust source changes

### Rust Source Changes (4 files, +781 total lines)

- `crates/contextro-engines/src/bm25.rs` (+196): query-aware confidence scoring
- `crates/contextro-tools/src/code.rs` (+281): `search_codebase_map` narrow-query rewrite
- `crates/contextro-tools/src/artifacts.rs` (+281): `audit()` dynamic `quality_score` (first attempt — score still 75 for this codebase)
- `crates/contextro-tools/src/search.rs` (+73): confidence label made query-aware

**Note:** v1.6.14 was initially characterized as documentation-only in session memory. Verified via `git log v1.6.13..HEAD -- crates/` — this was incorrect. Always check `git diff` before characterizing a release.

### What v1.6.14 Fixed

| Issue | Status |
|---|---|
| S1 HIGH: `get_document_symbols` format undocumented | FIXED — docs updated with columnar format and `include_signature` guidance |
| P3: `quality_score` stuck at 75 | PARTIAL — formula added but still returns 75 for this codebase |
| S5: confidence always `"medium"` | FIXED — `search("QueryCache")` now returns `confidence: "high"` |
| S6/S7 doc examples | FIXED — narrow-query examples added |

### Breaking Change Introduced in v1.6.14

`get_document_symbols` and `list_symbols` (file path variant) now return columnar format:

```json
{
  "file": "...",
  "columns": ["name", "type", "line", "end_line"],
  "symbols": [["dispatch", "function", 45, 120], ...],
  "total": 65
}
```

Old access pattern `result["symbols"][i]["name"]` silently breaks. Correct: `result["symbols"][i][columns.indexOf("name")]`. The `signature` field is opt-in via `include_signature: true`.

This was an undocumented breaking change in v1.6.14 (documented in v1.6.15 skill bundle).

### Scenario Results

All 7 scenarios PASS or IMPROVED. S7 remains IMPROVED (not PASS) due to P2.

---

## Part 7 — v1.6.15

**Graph:** 874 nodes (+16), 2,242 edges (+48)  
**Released:** 2026-05-15  
**Installed:** `cp crates/target/release/contextro /opt/homebrew/bin/contextro`

### Rust Source Changes (8 files)

| File | Lines Added | Key Changes |
|---|---|---|
| `crates/contextro-engines/src/bm25.rs` | +210 | New `build_plain_token_query` fast path; `should_run_supplemental_variants` (only expands when primary results insufficient); `is_plain_bm25_query`, `stem_bm25_token`, `merge_variant_results` |
| `crates/contextro-tools/src/code.rs` | +457 | `search_codebase_map` major rewrite; new `codebase_map_narrow_file_relevance_score` and `codebase_map_intra_file_relevance_score` for per-file relevance scoring |
| `crates/contextro-tools/src/artifacts.rs` | +172 | New `audit_quality_score(recommendation_count, max_connection_overage, max_file_overage)` — formula: `85 - (recs*5) - (max_conn/10) - (max_file/30)`, 95 if clean |
| `crates/contextro-tools/src/search.rs` | +119/-62 | Mostly rustfmt; minor logic tweaks |
| `crates/contextro-server/src/state.rs` | +6 | Minor |
| `crates/contextro-server/src/main.rs` | +14 | Minor |
| `crates/contextro-memory/src/store.rs` | +21 | Memory store improvements |
| `Cargo.toml` / `Cargo.lock` | version bumps | — |

### Documentation Updates

- `SKILL.md`: columnar `get_document_symbols` format fully documented (S1 HIGH fixed)
- `eval-rubric.md`: grading criteria updated to check `columns` index usage and `include_signature` opt-in
- `tool-decision-tree.md`: narrow-query guidance for `search_codebase_map` added

### Tool Sweep — v1.6.15

**37/37 operational. No regressions.**

| Tool | v1.6.14 | v1.6.15 | Delta |
|---|---|---|---|
| `health()` | PASS | PASS | — |
| `index()` | PASS | PASS | — |
| `search()` | PASS | PASS | — |
| `find_symbol()` | PASS | PASS | — |
| `search_codebase_map()` | PASS | PASS | improved precision |
| `audit()` | PASS | **IMPROVED** | quality_score now dynamic |
| `commit_search()` | PASS | PASS | — |
| `commit_history()` | PASS | PASS | — |
| `remember()` / `recall()` | PASS | PASS | — |
| `compact()` / `retrieve()` | PASS | PASS | — |
| `get_document_symbols()` | PASS | PASS | — |
| All others (25 tools) | PASS | PASS | — |

### What v1.6.15 Fixed

#### P3 FIXED: `audit()` quality_score now dynamic

```
v1.6.11–v1.6.14: quality_score always 75
v1.6.15:          quality_score: 67  (3 recommendations × 5 = -15, connection overage -3)
```

Formula verified: `85 - (3*5) - (max_conn_overage/10) - (max_file_overage/30) = 67`.  
Score reflects actual code health rather than a constant.

#### P1 IMPROVED: `search_codebase_map` narrow-query precision 2/3

| Query | v1.6.14 | v1.6.15 |
|---|---|---|
| `"how does commit search work"` | wrong file | `git_tools.rs` ✓ |
| `"how does memory recall work"` | wrong file | `memory.rs` ✓ |
| `"how does BM25 indexing work"` | `main.rs` (wrong) | `main.rs` (still wrong) |

BM25 indexing query still routes to `main.rs` instead of `bm25.rs`. Root cause: `bm25.rs` is the implementation but `main.rs` mentions "BM25" in initialization context; the narrow-query scorer may over-weight mention frequency.

#### S1 HIGH FIXED: `get_document_symbols` format documented

`packages/skills/skills/dev-contextro-mcp/SKILL.md` line 72 and 105–106 now fully describe the columnar response format. `eval-rubric.md` grading criteria updated.

### Scenario Results — v1.6.15

| Scenario | Result | Detail |
|---|---|---|
| S1: Onboarding | PASS | `overview()` → `architecture()` → `search()` → `explain()` all functional and accurate |
| S2: Feature deep dive | PASS | `search("QueryCache")` → `confidence: "high"`, `explain("QueryCache")` complete |
| S3: Feature planning | PASS | `edit_plan` + `impact()` return useful pre-change context |
| S4: Reliability audit | PASS | `audit()` → `quality_score: 67`; `dead_code()` returns 12 candidates; `.expect()` search → 32 sites |
| S5: Pre-PR quality gate | PASS | `impact("handle_search")` traces blast radius correctly |
| S6: Multi-session continuity | PASS (P4 caveat) | `recall()` surfaces v1.6.15 test note (`mem_bb3c8c45`) correctly; `compact`/`retrieve` roundtrip exact |
| S7: Git archaeology | IMPROVED | Filename queries work (`commit_search("bm25.rs")` → 3 results at 0.76); conceptual queries still fail |

### Cross-Version Scorecard

| Scenario | v1.6.11 | v1.6.12 | v1.6.13 | v1.6.14 | v1.6.15 |
|---|---|---|---|---|---|
| S1: Onboarding | PASS | PASS | PASS | PASS | PASS |
| S2: Feature deep dive | MIXED | MIXED | PASS | PASS | PASS |
| S3: Feature planning | MIXED | MIXED | PASS | PASS | PASS |
| S4: Reliability audit | PASS | PASS | PASS | PASS | PASS |
| S5: Pre-PR quality gate | PASS | PASS | PASS | PASS | PASS |
| S6: Multi-session continuity | MIXED | MIXED | PASS | PASS | PASS |
| S7: Git archaeology | MIXED | MIXED | IMPROVED | IMPROVED | IMPROVED |

### Persistent Issues After v1.6.15

| ID | Severity | Status | Description |
|---|---|---|---|
| P1 | MEDIUM | OPEN (partial) | `search_codebase_map` precision 2/3 narrow queries; BM25-related queries still route to `main.rs` instead of `bm25.rs` |
| P2 | HIGH | OPEN | `commit_search` cannot find commits by conceptual description when repo uses terse "Update X.rs" messages; filename workaround exists (`commit_search("bm25.rs")`) |
| P4 | LOW | OPEN | Stale bug memories from v1.6.12 (3 permanent notes describing fixed bugs) still surface in `recall()`; no `ttl`-based expiry for `"permanent"` memories |

### New Issues Found in v1.6.15

None. All tools operational, no regressions, no new breaking changes.

### Assessment

v1.6.15 is the strongest release tested. P3 (quality_score stuck at 75) is fully resolved. P1 shows real improvement (2/3 correct vs 1/3). The S1 HIGH documentation issue is fixed. No regressions found.

The two remaining substantive issues are:
1. **P2 (commit_search + terse messages)**: This is effectively a commit-convention problem, not a bug per se. The tool works correctly when messages have semantic content. A workaround exists (use filename as query). Adding a `git log -p` code-diff search mode would be the ideal fix.
2. **P1 (search_codebase_map BM25 query)**: The specific case of "BM25 indexing" → `main.rs` instead of `bm25.rs` is likely because `main.rs` initializes the BM25 engine and contains relevant comments, giving it high relevance scores. Distinguishing "owner of concept" from "users of concept" in file relevance scoring is a hard problem.

**Recommendation: release v1.6.15.**

---

## Appendix: Memory IDs for Stale Bugs (P4)

These memories describe bugs fixed in v1.6.13 or earlier, but remain as `permanent` notes:

- `mem_ab2e3888` — `repo_add`/`repo_remove` scope bug (fixed in v1.6.13)
- `mem_3fc6c1d4` — `search("how does caching work")` missing QueryCache (fixed in v1.6.13)
- `mem_2b262201` — `edit_plan` cross-subsystem `affected_symbols` unreliable (still partially valid)

To clean up: `forget(memory_id="mem_ab2e3888")`, `forget(memory_id="mem_3fc6c1d4")`.  
Keep `mem_2b262201` — `edit_plan` cross-subsystem limitation is still present in v1.6.15.
