# Dev Contextro MCP Eval Rubric

Use this rubric to judge whether the skill matches Anthropic's skill guidance and the
local `skills-guide.pdf`.

## Source Principles

- Frontmatter must say what the skill does and when to use it.
- `SKILL.md` should stay concise; detailed guidance belongs in `references/`.
- Test three things separately: triggering, functional execution, and performance
  comparison against the no-skill baseline.
- Prefer problem-first routing. Users ask for outcomes; the skill should pick the right
  Contextro tool sequence.

## Pass Criteria

### Triggering

- Triggers on obvious Contextro requests.
- Triggers on paraphrased versions of those requests.
- Does not trigger on unrelated coding or knowledge tasks.
- Does not over-trigger on direct single-file reads or from-scratch coding requests.

### Functional Routing

- Uses `find_symbol` for exact symbols.
- Uses `search` for concepts and `bm25` for exact identifiers.
- Uses `impact` before rename, delete, or signature-change guidance.
- Treats `index()` returning `status: "done"` as sufficient readiness for normal flows.
- Uses `status()` only when readiness is uncertain or for session re-entry checks.
- Uses `retrieve(ref_id)` for archive recovery after `compact()`.
- Uses `recall()` for durable memories created by `remember()`.
- Uses AST operations with `dry_run=True` before applying structural rewrites.

### Response Format Interpretation

- Reads full-key search payloads: `query`, `confidence`, `results`, `total`, and usually `limit` plus `truncated`.
- Reads search results with `name`, `file`, `line`, `type`, `score`.
- Reads `find_symbol` and symbol lookup responses as `{ symbols: [...], total: N }`.
- Reads `retrieve()` responses as `{ ref_id, content }`.
- Reads `{callers: [...]}` and `{callees: [...]}` directly from find_callers/find_callees.

### Anti-Patterns

- No repeated re-indexing.
- No mandatory `status()` call after every successful `index()`.
- No serial `find_symbol` calls when `lookup_symbols` is better.
- No `remember()` in place of `compact()` for pre-compaction archival.
- No claim that `recall(memory_type="archive")` retrieves archives.
- No `search_codebase_map` for narrow single-symbol questions when `find_symbol` + `focus`/`explain` is more direct.
- No shell `git log` when Contextro history tools answer the question.

### Performance Comparison

- Fewer tool calls than the no-skill baseline for orientation, safe refactor checks, and
  bug investigation.
- Fewer file reads than the no-skill baseline.
- Lower token usage by preferring Contextro tool outputs over file-heavy baseline workflows.

## Recommended Thresholds

- Relevant-query trigger rate: at least 90 percent.
- Unrelated-query non-trigger rate: 100 percent.
- `impact()` before refactor guidance: 100 percent.
- `index()` readiness interpreted correctly: at least 95 percent.
- Archive vs memory recovery distinction: at least 95 percent.
- Performance improvement vs baseline: clear reduction in reads, shell search, and token-heavy workflows.
