# Dev Contextro MCP Eval Rubric

Use this rubric to judge whether the skill matches Anthropic's skill guidance and the
local `skills-guide.pdf`.

## Source Principles

- Frontmatter must say what the skill does and when to use it.
- `SKILL.md` should stay concise; detailed guidance belongs in `references/`.
- The bundle should teach the full 40-tool public surface plus the eight `code(...)`
  operations through progressive disclosure.
- Test three things separately: triggering, functional execution, and performance
  comparison against the no-skill baseline.
- Prefer problem-first routing. Users ask for outcomes; the skill should pick the right
  Contextro tool sequence.
- Benchmark and latency claims must match the retained measurements in
  `references/benchmark-results.md`.

## Pass Criteria

### Triggering

- Triggers on obvious Contextro requests.
- Triggers on paraphrased versions of those requests.
- Does not trigger on unrelated coding or knowledge tasks.
- Does not over-trigger on direct single-file reads or from-scratch coding requests.

### Functional Routing

- Uses `health()` only when server/runtime availability itself is uncertain.
- Uses `status()` only when active-repo, index, or session-state details are uncertain.
- Uses `find_symbol` for exact symbols.
- Uses `search` for concepts and `bm25` for exact identifiers.
- Distinguishes `search`, `find_symbol`, `code(search_symbols)`, and
  `code(lookup_symbols)` correctly.
- Uses `impact` before rename, delete, or signature-change guidance.
- Uses `refactor_check` for one-shot pre-edit analysis when the task centers on one
  symbol.
- Uses `completion_check` after rename/signature-change claims when the user asks whether
  all callers were updated.
- Treats `index()` returning `status: "done"` as sufficient readiness for normal flows.
- Uses `repo_add`, `repo_status`, and `repo_remove` for cross-repo workflows.
- Uses `retrieve(ref_id)` for archive recovery after `compact()`.
- Uses `restore()` for project re-entry summaries after restart or time away.
- Uses `session_snapshot()` for raw recent events and exact prior tool-call context.
- Uses `recall()` for durable memories created by `remember()`.
- Uses `knowledge()` with the correct command shape for add/search/show/list/update/remove.
- Uses `audit`, `docs_bundle`, and `sidecar_export` only when the user wants generated
  artifacts.
- Uses `introspect(query=...)` or `introspect(tool=...)` when the agent is unsure which
  tool or parameters fit.
- Uses `skill_prompt()` only for bootstrap text for another agent/client.
- Uses AST operations with `dry_run=true` before applying structural rewrites.

### Response Format Interpretation

- Reads full-key search payloads: `query`, `confidence`, `results`, `total`, and usually `limit` plus `truncated`.
- Reads search results with `name`, `file`, `line`, `type`, `score`.
- Reads `find_symbol` and symbol lookup responses as `{ symbols: [...], total: N }`.
- Reads `get_document_symbols` and file-path `list_symbols` responses as columnar `{ file, columns, symbols, total }`, using `columns` indexes instead of assuming `symbols[i].name`.
- Knows `signature` is opt-in via `include_signature=true` and `end_line` appears only when needed.
- Distinguishes file-path `list_symbols` from directory-path `list_symbols`, whose directory contract remains object rows with `callers` and `callees`.
- Reads `retrieve()` responses as `{ ref_id, content }`.
- Reads `{callers: [...]}` and `{callees: [...]}` directly from find_callers/find_callees.

### Tool Coverage

- Evals and release validation together cover every public tool family:
  readiness/repo scope, discovery/graph, refactor safety/completion, `code(...)`
  operations, quality/export tools, git history, memory/knowledge/recovery, and
  self-documentation helpers.
- At least one eval explicitly exercises the nearby-tool distinctions that commonly cause
  misuse: `health` vs `status`, `retrieve` vs `restore` vs `session_snapshot` vs
  `recall`, and `impact` vs `refactor_check` vs `completion_check`.

### Anti-Patterns

- No repeated re-indexing.
- No mandatory `status()` call after every successful `index()`.
- No `health()` when only index or active-repo state is needed.
- No serial `find_symbol` calls when `lookup_symbols` is better.
- No `remember()` in place of `compact()` for pre-compaction archival.
- No claim that `recall(memory_type="archive")` retrieves archives.
- No `search_codebase_map` for narrow single-symbol questions when `find_symbol` + `focus`/`explain` is more direct.
- No `restore()` or `session_snapshot()` in place of `retrieve(ref_id)` for compacted archives.
- No `docs_bundle()` or `sidecar_export()` for one-off factual answers.
- No `skill_prompt()` for routine code discovery or analysis.
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
- `completion_check()` after caller-completeness claims: at least 95 percent.
- `index()` readiness interpreted correctly: at least 95 percent.
- `health()` vs `status()` distinction: at least 95 percent.
- Archive vs restore vs memory recovery distinction: at least 95 percent.
- Public tool-family coverage in the release bundle eval set: 100 percent.
- Performance improvement vs baseline: clear reduction in reads, shell search, and token-heavy workflows.
