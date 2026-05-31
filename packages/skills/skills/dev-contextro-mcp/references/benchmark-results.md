# Benchmark Results

Current evidence in this bundle is limited to studies and repo files present in this working tree.

## Validated Studies

### Contextro repo study, 200 tasks, current refresh

Measured on this repository with the deterministic `contextro-study` harness.

| Metric | Contextro | stronger_local | Delta |
|---|---|---|---|
| Success rate | 100% | 93.0% | +7.0 points |
| Total tokens | 5,412 | 225,153 | 97.6% reduction |
| Mean tokens per task | 27.06 | 1,125.77 | lower |
| Tool calls per task | 1.0 | 3.045 | lower |
| Files read | 0 | 369 | eliminated |

Source: current validated `contextro-study` figures for this working tree.

### Current repo study by task family

All rows below come from the same refreshed `200`-task `contextro-study` run on this repository.

| Family | Tasks | Contextro success | stronger_local success | Contextro tokens | stronger_local tokens | Reduction |
|---|---|---|---|
| `batch_lookup` | 40 | 100% | 90.0% | 2,034 | 57,159 | 96.4% reduction |
| `document_symbols` | 40 | 100% | 100% | 1,507 | 66,994 | 97.8% reduction |
| `exact_search` | 60 | 100% | 95.0% | 788 | 59,283 | 98.7% reduction |
| `symbol_discovery` | 60 | 100% | 88.3% | 1,083 | 41,717 | 97.4% reduction |

### Published repo-root README study, production TypeScript monorepo, 1,000 tasks

This is the published study already cited in the repo root README.

| Metric | Baseline | Contextro | Delta |
|---|---|---|---|
| Success rate | 99.5% | 100% | +0.5 point |
| Total tokens | 941,748 | 93,819 | 90% reduction |
| Median latency | 199.8ms | 0.081ms | 2,466x faster |
| Tool calls per task | 3.2 | 1.0 | lower |
| Files read | 1,961 | 0 | eliminated |

Source: `README.md`

## Latest Release-Bench Refresh On This Repo

The latest `contextro-bench` refresh on this repository covered the full public tool surface.

| Metric | Actual | Target | Status |
|---|---|---|---|
| Cold index average | 43.53ms | `<= 40ms` | open |
| Search latency average | 110.6us | `<= 137us` | pass |
| MCP throughput | 6911/s | `>= 500/s` | pass |
| Tool coverage | 40/40 tools, 41 cases | full surface | pass |

Current hotspot references from that refresh:

- `test_for`: `0.28ms` average
- `diff_preview`: `0.79ms` average
- `repo_add`: `1.19ms` average
- `repo_rm_active`: `4.47ms` average
- `docs_bundle`: `0.37ms` average
- `sidecar_export`: `0.13ms` average

## Guardrails For Future Runs

- Keep the `200`-task `contextro-study` success rate at `100%` on this repo when shipping compact-response changes.
- Treat `5,412` total Contextro tokens as the current repo baseline on this repo.
- Do not lower the default `get_document_symbols` cap below `3`; the current study tasks expect `3` symbols.
- Keep the full-surface `contextro-bench` refresh green on search latency and throughput while continuing to push cold index back under the `<=40ms` target.
- Do not accept meaningful `test_for`, `diff_preview`, `repo_add`, `repo_rm_active`, `docs_bundle`, or `sidecar_export` regressions for marginal token wins.

## Current Runtime Contracts

### Search

`search()` still uses long-form keys, but exact single-symbol hits are compacted:

```json
{
  "query": "QueryCache",
  "confidence": "high",
  "results": [
    {
      "name": "QueryCache",
      "file": "crates/contextro-engines/src/cache.rs",
      "line": 1,
      "score": 0.99
    }
  ],
  "total": 1,
  "limit": 10
}
```

Notes:
- `confidence` is present in current responses.
- Search results always use `name`, `file`, `line`, and `score`.
- `type` is omitted for unique exact-symbol hits and kept for broader or ambiguous results.
- `file` is repo-relative when Contextro can infer a single repo root from the result set, even without an explicit `codebase`.
- `truncated` is omitted unless `total > limit`.
- Response truncation and budgeting are handled by the server wrapper's `max_tokens`, not `context_budget`.

### Document symbols

`code(operation="get_document_symbols")` keeps the columnar contract but defaults to a compact payload:

```json
{
  "file": "src/main.py",
  "columns": ["name", "type", "line"],
  "symbols": [
    ["fn_0", "function", 1],
    ["fn_1", "function", 4],
    ["fn_2", "function", 7]
  ],
  "total": 30,
  "truncated": true
}
```

Notes:
- When `include_signature=false`, the default response is capped at `3` rows.
- Pass `limit` to override the compact default.
- `include_signature=true` keeps the columnar contract and bypasses the default `3`-row cap.

### Symbol lookup

`code(operation="lookup_symbols")` uses wrapper objects such as:

```json
{
  "symbols": [
    {
      "name": "hello",
      "file": "module.py",
      "line": 1
    }
  ],
  "total": 1
}
```

Notes:
- `lookup_symbols` omits `type` for unique exact matches.
- `lookup_symbols` keeps `type` for ambiguous matches and when `include_source=true`.
- `find_symbol()` still uses the broader symbol record contract and keeps `type`.

### Archive retrieval

`retrieve()` accepts only `ref_id` and returns:

```json
{
  "ref_id": "arc_ab12cd34",
  "content": "archived session content"
}
```

## Guidance For Claims

Use the refreshed `200`-task repo study and the published `1,000`-task README study for external-facing benchmark claims in this skill bundle.
Do not cite older `100`-task repo numbers, compact-key token tables, MRR claims, or sandbox-response behavior unless a current repo source is added that supports them.
