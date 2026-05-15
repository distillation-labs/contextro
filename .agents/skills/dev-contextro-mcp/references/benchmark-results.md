# Benchmark Results

Current evidence in this bundle is limited to studies and repo files present in this working tree.

## Validated Studies

### Contextro repo study, 100 tasks

Measured on this repository.

| Metric | Contextro | stronger_local | Delta |
|---|---|---|---|
| Success rate | 100% | 99% | +1 point |
| Total tokens | 9,905 | 109,067 | 90.9% reduction |
| Tool calls per task | 1.0 | 3.03 | lower |
| Files read | 0 | 183 | eliminated |

Source: current validated `contextro-study` figures for this working tree.

### Contextro repo study, 200 tasks

Measured on this repository.

| Metric | Contextro | stronger_local | Delta |
|---|---|---|---|
| Success rate | 100% | 99% | +1 point |
| Total tokens | 23,447 | 222,646 | 89.5% reduction |
| Tool calls per task | 1.0 | 2.89 | lower |
| Files read | 0 | 335 | eliminated |

Source: current validated `contextro-study` figures for this working tree.

Useful category notes from the 200-task study:

| Category | Token reduction |
|---|---|
| `batch_lookup` | 94.4% |
| `document_symbols` | 83.1% |
| `exact_search` | 87.2% |
| `symbol_discovery` | 94.9% |

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

## Current Runtime Contracts

### Search

`search()` returns full-key responses:

```json
{
  "query": "authentication middleware",
  "confidence": "high",
  "results": [
    {
      "name": "AuthMiddleware",
      "file": "src/auth.rs",
      "line": 42,
      "type": "struct",
      "score": 0.9123
    }
  ],
  "total": 1,
  "limit": 10,
  "truncated": false
}
```

Notes:
- `confidence` is present in current responses.
- Search results use `name`, `file`, `line`, `type`, `score`, not compact keys.
- Response truncation and budgeting are handled by the server wrapper's `max_tokens`, not `context_budget`.

### Symbol lookup

`find_symbol()` and `code(operation="lookup_symbols")` use wrapper objects such as:

```json
{
  "symbols": [
    {
      "name": "AuthMiddleware",
      "type": "struct",
      "file": "src/auth.rs",
      "line": 42
    }
  ],
  "total": 1
}
```

### Archive retrieval

`retrieve()` accepts only `ref_id` and returns:

```json
{
  "ref_id": "arc_ab12cd34",
  "content": "archived session content"
}
```

## Guidance For Claims

Use the study tables above for external-facing benchmark claims in this skill bundle.
Do not cite older compact-key token tables, MRR claims, or sandbox-response behavior unless a current repo source is added that supports them.
