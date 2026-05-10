# Contextro Edit Assistance Plan

**Date:** 2026-05-10  
**Status:** Implemented baseline + validated benchmark  
**Goal:** Help agents find, plan, preview, and safely apply edits without turning Contextro into a generic patch editor

## Executive Summary

Contextro should stay the discovery and safety layer.

The right edit flow is:

`locate -> scope -> plan -> preview -> apply -> verify -> resume`

Research across OpenAI, Anthropic, Cursor, Mistral, Aider, and code-editing papers points to the same pattern:

- Find the right code first.
- Keep the edit representation structural, not brittle.
- Preview before applying.
- Verify after applying.
- Persist the plan and state so long-running sessions do not lose the thread.

The practical implication for Contextro is simple: add an explicit edit-planning layer on top of the tools already here, then use the existing AST rewrite path as the safe apply engine.

## Research Basis

### Facts

- OpenAI says to give Codex "a map, not a manual" and treats repository knowledge as the system of record. That strongly favors compact repo maps, structured docs, and enforceable invariants over giant prompt blobs. [1][2]
- Anthropic's context management work separates context editing from memory, and its long-running-agent harness uses an initializer agent, progress files, and git history to survive fresh context windows. [3][4]
- Cursor shows that semantic search, dynamic context discovery, and turning long outputs into files improve agent performance and reduce token waste. [5][6][7]
- Mistral splits coding into two useful modes: Codestral for FIM/code completion and Devstral for agentic tool use and multi-file software engineering tasks. [8][9]
- Aider shows that edit format matters. Unified diffs and flexible patch application outperform naive whole-file or line-number-heavy approaches for many code-editing tasks. [10]
- Code-editing benchmarks consistently show that editing is a distinct capability from synthesis. Instruction-tuning and structure-aware training help, but repo-scale navigation and patch construction remain separate problems. [11][12][13][14][15][16]

### Inferences

- Contextro already covers most of the discovery side. What it lacks is an explicit edit lifecycle artifact.
- The safest edit scope for Contextro is structural, preview-first, and repo-aware.
- The most useful new surface is not a free-form patch API. It is an edit-plan API that explains what should change, where, why, and how to verify it.
- Multi-file changes will need a plan artifact and session continuity because the actual edit may span several turns and several tools.

### Hypotheses

- A planning-first edit flow will reduce failed edit attempts and repeated prompting.
- AST-backed dry-run previews will be more reliable than line-numbered diffs for Contextro's core language coverage.
- Persisting edit plans in repo-local artifacts and session snapshots will make long edits survive compaction and resumption.
- A narrow, structural edit system will be more maintainable than a generic patch ingester.

## Current Baseline

Contextro already has the core primitives needed for an edit-assistance workflow:

| Surface | Current state | Evidence |
|---|---|---|
| Discovery | `search_symbols`, `lookup_symbols`, `get_document_symbols`, `pattern_search`, `generate_codebase_overview`, `search_codebase_map` | `src/contextro_mcp/server.py` |
| Structural apply | `pattern_rewrite` with `dry_run=True` preview and `dry_run=False` apply | `src/contextro_mcp/server.py` |
| Blast radius | `impact()` and caller/callee navigation | `src/contextro_mcp/server.py`, `src/contextro_mcp/memory/session_tracker.py` |
| Context continuity | `session_snapshot()`, `compact()`, archive recall | `src/contextro_mcp/server.py`, `src/contextro_mcp/memory/session_tracker.py`, `src/contextro_mcp/memory/compaction_archive.py` |
| Large results | `OutputSandbox` plus `retrieve()` for on-demand expansion | `src/contextro_mcp/engines/output_sandbox.py`, `src/contextro_mcp/server.py` |
| Architecture view | `generate_codebase_overview()` and `search_codebase_map()` | `src/contextro_mcp/server.py` |
| Constraints | Single local MCP server, local-first, target <350MB RSS | `docs/ARCHITECTURE.md`, `AGENTS.md` |

Current workflow baselines worth preserving while adding edit support:

- `smart` chunking remains the best balanced retrieval default in the current research notes.
- `search` and `explain` already stay token-efficient.
- `session_snapshot()` already compresses working context into a compact recovery artifact.

Current gap before implementation:

- `pattern_rewrite` was a good structural mutator, but the repo lacked a first-class edit plan artifact that bridged discovery to apply.
- The `pattern_rewrite` contract needed normalization and tests because repo docs and eval notes had historically disagreed about `file_path` vs `path` behavior.

Current shipped baseline:

- `code(operation="edit_plan")` now returns scoped target files, target symbols, related tests, risks, rollback guidance, and verify commands.
- `pattern_rewrite` now returns unified-diff previews, touched lines, changed symbols, preview signatures, and supports `file_path`, file `path`, and directory `path` consistently.
- Session observability now records `edit_plan`, `edit_preview`, and `edit_apply` events in `session_snapshot()` and `status()`.
- Optional preview-before-apply enforcement is available via `CTX_EDIT_REQUIRE_PREVIEW_BEFORE_APPLY=true`.

## Implemented Checklists

### Phase 0

- [x] Added a curated edit benchmark in `scripts/benchmark_edit_assistance.py`.
- [x] Added direct tests for `pattern_rewrite` contract cases in `tests/test_editing.py`.
- [x] Verified `file_path` single-file, `path` single-file, and directory-scoped rewrite behavior.
- [x] Captured baseline token-efficiency guardrail results with `python3.11 scripts/benchmark_token_efficiency.py`.

### Phase 1

- [x] Added `edit_plan` under the existing `code` tool.
- [x] Returned repo-relative targets, risks, verify labels, verify commands, and rollback hints.
- [x] Added focused tests for single-file and directory-scoped planning.
- [x] Benchmarked planning outcomes with deterministic scoring.

### Phase 2

- [x] Switched rewrite previews to unified diffs.
- [x] Added changed-symbol and touched-line metadata.
- [x] Added preview signatures to pair dry-run and apply calls.
- [x] Added optional preview-before-apply enforcement.

### Phase 3

- [x] Covered directory and multi-file scoped rewrites in the benchmark suite.
- [x] Returned ordered `apply_sequence` and grouped target paths from `edit_plan`.
- [x] Stored edit lifecycle state in `session_snapshot()` for resumability.

### Phase 4

- [x] Exposed edit telemetry in `status()` and `session_snapshot()`.
- [x] Added a deterministic scoring rubric and invariant structure in `src/contextro_mcp/editing/benchmarking.py`.
- [x] Verified no focused regression in token efficiency or packaged tool tests.

## Measured Result

### Edit benchmark

- Command: `python3.11 scripts/benchmark_edit_assistance.py`
- Current result: `6/6` tasks passed, `mean_score = 100.0`
- Covered flows:
  - single-file planning
  - single-file preview
  - single-file apply
  - directory/multi-file planning
  - directory preview
  - directory apply

### Guardrails

- Command: `python3.11 scripts/benchmark_token_efficiency.py`
- Result: unchanged from the measured baseline for existing token metrics.
- Focused regressions stayed green:
  - `python3.11 -m pytest -q tests/test_editing.py tests/test_tools_basic.py tests/test_product_tools.py tests/test_packaged_graph_reports.py tests/test_cli.py`
  - Result: `53 passed`

## Exact Benchmark Schema and Rubric

Implemented in `src/contextro_mcp/editing/benchmarking.py`.

- Schema surfaces:
  - `EditBenchmarkTask`
  - `EditOperation`
  - `EditConstraints`
  - `EditExpected`
  - `EditBenchmarkRun`
  - `TextAssertion`
- Scoring helpers:
  - `rank_score()`
  - `set_f1()`
  - `count_score()`
  - `assertion_score()`
  - `order_score()`
  - `parity_score()`
  - `score_run()`

Implemented thresholds:

- `plan_only >= 85`
- `preview >= 90`
- `apply >= 95`
- `multi_file_plan >= 85`

## What "Edit" Means Here

Contextro should support three distinct edit modes:

1. `plan` - no file writes, only target selection, blast-radius analysis, and proposed edit strategy.
2. `preview` - AST-safe dry-run output that shows what would change.
3. `apply` - an explicit, verified structural rewrite.

That split matters because the hard part is not just writing code. It is deciding what should change, proving the choice is safe, and carrying the context across turns.

## Proposed User Flow

1. Agent asks Contextro to orient on the repo.
2. Contextro returns a compact map: files, symbols, graph shape, likely hot spots.
3. Agent asks for impact and candidate targets.
4. Contextro returns an `edit_plan` object with scope, risks, and verification steps.
5. Agent runs a preview using `pattern_rewrite(dry_run=True)` or the plan references a preview artifact.
6. Agent reviews the preview, then applies the rewrite.
7. Agent runs syntax and test checks.
8. Contextro stores the plan and state in session snapshot or archive so the edit can be resumed or rolled back.

## Adopt / Adapt / Avoid

| Strategy | Verdict | Why |
|---|---|---|
| Repo map + scope first | Adopt | OpenAI, Cursor, and Anthropic all show that compact repo knowledge beats huge manuals. |
| Structured edit plan artifact | Adopt | This is the missing bridge between discovery and apply. |
| AST-backed dry-run preview | Adopt | Best fit for safe, syntax-aware edits. |
| `pattern_rewrite` as the core mutator | Adopt | Already exists and matches the safe structural-edit strategy. |
| FIM-style insertion for local completions | Adapt | Useful for small continuations, but not enough for repo-scale edits by itself. |
| Unified diff export for compatibility | Adapt | Good as an interchange format, not as the primary apply mechanism. |
| Generic arbitrary patch ingest | Avoid | Too brittle, too easy to misapply, and too close to raw text surgery. |
| One giant edit prompt | Avoid | Context bloat, stale instructions, and poor recoverability. |
| Cloud-first edit state | Avoid | Conflicts with Contextro's local-first, privacy-first design. |

## Phase Plan

### Phase 0 - Contract Cleanup and Baseline

**Goal:** Define what an edit is, what success looks like, and what current behavior is already safe.

**Ship:**

- A small curated edit benchmark set for the current codebase.
- A normalized `pattern_rewrite` contract in docs and tests.
- Baseline measurements for planning time, preview time, apply success, syntax validity, and token use.

**How it works:**

- Use `search_codebase_map()` and `generate_codebase_overview()` to find likely edit surfaces.
- Use `impact()`, `explain()`, and `find_callers()` before any signature or rename change.
- Record session state with `session_snapshot()` before and after the edit attempt.

**Exit criteria:**

- At least 20 representative edit tasks are defined.
- `pattern_rewrite` contract is unambiguous in code, docs, and evals.
- Baseline metrics are captured for all tasks.

**Risk:**

- If this phase is skipped, later edit metrics will be noisy and hard to trust.

### Phase 1 - Plan-Only Preflight

**Goal:** Add a first-class edit plan without writing files.

**New artifact:** `edit_plan`

**Suggested shape:**

```json
{
  "goal": "...",
  "edit_kind": "rename|insert|refactor|multi_file",
  "target_files": ["..."],
  "target_symbols": ["..."],
  "impact": ["..."],
  "recommended_operation": "pattern_rewrite",
  "tests": ["..."],
  "risks": ["..."],
  "rollback": "git revert / restore pre-edit state",
  "confidence": 0.0
}
```

**Uses existing Contextro surfaces:**

- `search_symbols`, `lookup_symbols`, `get_document_symbols`
- `pattern_search`
- `impact`
- `generate_codebase_overview`
- `search_codebase_map`
- `commit_search`
- `session_snapshot`

**What this should solve:**

- Where should the edit land?
- Which symbols and files are in play?
- What else breaks if we do this?
- What test or lint command should be run next?

**Exit criteria:**

- For a curated edit set, the plan identifies the correct target file or symbol in the top 1 to 3 candidates most of the time.
- The plan includes blast radius and verification steps.
- The plan is small enough to live comfortably in a tool response or archive reference.

**What to avoid:**

- No file writes.
- No silent broad rewrites.
- No reliance on hidden conversational memory only.

### Phase 2 - Previewed Structural Apply

**Goal:** Make the existing AST rewrite path the explicit preview-and-apply engine.

**Ship:**

- Better `pattern_rewrite(dry_run=True)` previews.
- Preview output that includes changed symbols, touched lines, and a compact before/after view.
- Consistent behavior for single-file and directory-scoped rewrites.
- Clear guidance that `dry_run=True` is the default safe path.

**How it works:**

- The plan identifies candidate files and AST patterns.
- `pattern_rewrite` runs in dry-run mode first.
- The preview is stored in the output sandbox so the agent can inspect only the relevant parts.
- Once reviewed, the same structural rewrite is applied.

**Exit criteria:**

- Dry-run preview and applied result match for the curated set.
- Syntax remains valid after apply.
- The preview is informative enough that the agent does not need to re-read the whole file.

**Why this phase matters:**

- Aider and recent structure-aware edit research both show that the representation of the edit is a major determinant of reliability.
- Contextro should lean into structure, not line-by-line patching.

### Phase 3 - Multi-File Edit Bundles

**Goal:** Support edits that span several files without losing the thread.

**Ship:**

- A grouped edit plan that sequences related rewrites.
- Plan state stored in session snapshot or archive.
- Explicit dependency order for multi-file changes, for example interface first, implementations second, docs and tests last.

**How it works:**

- The plan surfaces all affected files and symbols.
- The agent executes a sequence of structural rewrites, not one giant mutation.
- Contextro keeps the working set compact with `session_snapshot()` and `compact()`.
- Large previews live in the sandbox and are retrieved only when needed.

**Exit criteria:**

- Multi-file edits can be resumed after compaction.
- The plan survives across turns.
- Rollback is straightforward because the sequence and targets are explicit.

**What this should not become:**

- Not a generic repo-wide patch engine.
- Not a long-lived hidden state machine.

### Phase 4 - Hardening and Rollout

**Goal:** Make edit assistance reliable enough to use by default.

**Ship:**

- Telemetry for plan creation, preview, apply, verify, and rollback.
- A small set of edit failure classes, for example wrong target, syntax failure, test failure, broad unintended rewrite.
- Feature flags for plan-only, preview, and apply paths.
- Documentation that tells agents when to stop at plan-only and when to apply.

**Exit criteria:**

- Metrics are stable across the benchmark set.
- The safe path is the default path.
- The fallback path remains discovery-only if the plan quality is low.

## Harness and Eval Plan

### Existing benchmarks to preserve

- `python scripts/benchmark_retrieval_quality.py --path src --query-limit 20`
- `python scripts/benchmark_chunk_profiles.py --path src --query-limit 20`
- `python scripts/benchmark_token_efficiency.py`
- `python scripts/benchmark_disclosure.py`
- `python scripts/bench_final.py`
- `pytest -v`
- `ruff check .`

### New edit benchmark

1. `python3.11 scripts/benchmark_edit_assistance.py`

### Metrics

| Metric | Meaning | Target |
|---|---|---|
| Plan accuracy | Did the plan pick the right file or symbol? | High enough that the plan is useful without manual rediscovery |
| Blast-radius accuracy | Did `impact()` match what actually broke? | High on signature and rename tasks |
| Preview/apply parity | Did dry-run match the applied change? | Near-perfect on the curated set |
| Syntax validity | Did the edited code parse? | 100 percent for the curated set |
| Test pass rate | Did the expected tests still pass? | No regression vs baseline |
| Token cost | How expensive was the edit lifecycle? | Lower than repeated grep/read loops |
| Time to first useful preview | How quickly does the agent get something reviewable? | Fast enough for interactive use |

### Success threshold from the implemented baseline

- Planning returns the correct scoped target set for the curated benchmark.
- Preview is reviewable without loading the whole file.
- Preview/apply parity is stable on the curated benchmark.
- No regression in the existing token-efficiency benchmark.

## Observability and Guardrails

- Record edit lifecycle events in the session tracker: `edit_plan`, `edit_preview`, `edit_apply`, `edit_verify`.
- Store large previews in the output sandbox and only retrieve on demand.
- Include the repo-relative file path in every plan and preview so path remapping survives local mount differences.
- Keep `dry_run=True` as the default safe entry point.
- Require `impact()` before rename, delete, or signature change operations.
- Require a syntax check, then a targeted test or lint command after apply.
- Keep all state local and versioned. Do not hide critical plan details only in chat.

## Rollout and Rollback

### Rollout

1. Land the current benchmarked baseline.
2. Keep `CTX_EDIT_REQUIRE_PREVIEW_BEFORE_APPLY=true` available for safer apply workflows.
3. Expand the curated benchmark set beyond Python print-replacement tasks.
4. Add explicit post-apply verification helpers if benchmark coverage widens.
5. Make preview-first edit assistance the default workflow only when the wider benchmarks hold.

### Rollback

- If planning is wrong, disable the edit-plan path and fall back to discovery-only tools.
- If preview/apply parity regresses, keep dry-run previews but disable apply by default.
- If multi-file bundles become brittle, fall back to single-file structural rewrites.
- If the system starts behaving like a generic patcher, narrow the scope again.

## Recommendation

Start with planning plus preview, not raw apply.

The best first version of Contextro editing is:

`discovery tools -> edit_plan -> AST dry-run preview -> AST apply -> verify -> session_snapshot`

That path now exists in the repo and is validated on a deterministic local benchmark with no measured regression on the current token-efficiency guardrail.

## Sources

1. OpenAI, "Harness engineering: leveraging Codex in an agent-first world" - https://openai.com/index/harness-engineering/
2. OpenAI, "Introducing GPT-5.2-Codex" - https://openai.com/index/introducing-gpt-5-2-codex/
3. Anthropic, "Managing context on the Claude Developer Platform" - https://claude.com/blog/context-management
4. Anthropic, "Effective harnesses for long-running agents" - https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents
5. Cursor, "Improving agent with semantic search" - https://cursor.com/blog/semsearch
6. Cursor, "Dynamic context discovery" - https://cursor.com/blog/dynamic-context-discovery
7. Cursor, "Fast regex search: indexing text for agent tools" - https://cursor.com/blog/fast-regex-search
8. Mistral AI, "Devstral" - https://mistral.ai/news/devstral
9. Mistral Docs, "Coding" - https://docs.mistral.ai/capabilities/code_generation
10. Aider, "Edit formats" and "Unified diffs make GPT-4 Turbo 3X less lazy" - https://aider.chat/docs/more/edit-formats.html , https://aider.chat/docs/unified-diffs.html
11. Cassano et al., "Can It Edit? Evaluating the Ability of Large Language Models to Follow Code Editing Instructions" - https://arxiv.org/abs/2312.12450
12. Guo et al., "CodeEditorBench: Evaluating Code Editing Capability of Large Language Models" - https://arxiv.org/abs/2404.03543
13. Li et al., "InstructCoder: Instruction Tuning Large Language Models for Code Editing" - https://aclanthology.org/2024.acl-srw.52/
14. LaBash et al., "RES-Q: Evaluating Code-Editing Large Language Model Systems at the Repository Scale" - https://arxiv.org/abs/2406.16801
15. Gong et al., "Evaluation of LLMs on Syntax-Aware Code Fill-in-the-Middle Tasks" - https://proceedings.mlr.press/v235/gong24f.html
16. Cheng et al., "To Diff or Not to Diff? Structure-Aware and Adaptive Output Formats for Efficient LLM-based Code Editing" - https://arxiv.org/abs/2604.27296
