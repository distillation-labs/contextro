# Benchmark Results

Current evidence in this bundle is limited to studies and repo files present in this working tree.

## Validated Studies

### Contextro repo study, 200 tasks, retained current state

Measured on this repository with the deterministic `contextro-study` harness.

| Metric | Contextro | stronger_local | Delta |
|---|---|---|---|
| Success rate | 100% | 97.5% | +2.5 points |
| Total tokens | 6,244 | 227,072 | 97.3% reduction |
| Mean tokens per task | 31.22 | 1,135.36 | lower |
| Tool calls per task | 1.0 | 2.96 | lower |
| Files read | 0 | 352 | eliminated |

Source: current validated `contextro-study` figures for this working tree, retained current state.

### Retained breakthrough vs pre-optimization Contextro baseline

These deltas are the benchmark guardrails for this repo.

| Metric | Baseline Contextro | Retained Contextro | Delta |
|---|---|---|---|
| Success rate | 100% | 100% | preserved |
| Total tokens | 20,851 | 6,244 | 70.1% reduction |
| `document_symbols` tokens | 10,805 | 1,854 | 82.8% reduction |
| `exact_search` tokens | 4,230 | 1,441 | 65.9% reduction |
| `batch_lookup` tokens | 3,568 | 1,674 | 53.1% reduction |
| `symbol_discovery` tokens | 2,248 | 1,275 | 43.3% reduction |

### Published repo-root README study, production TypeScript monorepo, 1,000 tasks

This is the published study already cited in the repo root README.

| Metric | Baseline | Contextro | Delta |
|---|---|---|---|
| Success rate | 99.5% | 100% | +0.5 point |
| Total tokens | 941,748 | 93,819 | 90% reduction |
| Median latency | 199.8ms | 0.081ms | 2,466x faster |
| Tool calls per task | 3.2 | 1.0 | lower |
| Files read | 1,961 | 0 | eliminated |

Source: `/Users/japneetkalkat/contextro/README.md`

## Guardrails For Future Runs

- Keep the `200`-task `contextro-study` success rate at `100%` on this repo when shipping compact-response changes.
- Treat `6,244` total Contextro tokens as the current retained baseline on this repo.
- Do not lower the default `get_document_symbols` cap below `3`; the current study tasks expect `3` symbols.
- Retained `contextro-bench` sanity rerun on this repo: cold index `31.9-32.6ms`, search `0.111-0.112ms`, code `0.27-0.28ms`, active-scope `repo_remove` restore `8.01-8.24ms`. Do not accept meaningful bench regressions for marginal token wins.

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

Use the retained `200`-task repo study and the published `1,000`-task README study for external-facing benchmark claims in this skill bundle.
Do not cite older `100`-task repo numbers, compact-key token tables, MRR claims, or sandbox-response behavior unless a current repo source is added that supports them.
