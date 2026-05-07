# Workflow Research

Updated: 2026-05-06

This note refreshes Contextia's workflow research using primary sources plus the
current repository state. It is intentionally opinionated: every recommendation
maps to a real surface in the codebase and is ranked by expected ROI for
Contextia's single-process, low-memory MCP design.

## Method

- Facts come from the cited sources or the current repo.
- Inferences connect those facts to Contextia's architecture.
- Hypotheses are proposed experiments, not established conclusions.
- For living product docs without immutable publication dates, the tables below
  use the access year `2026`.

## Executive Summary

- Contextia already has the right core architecture for local developer
  workflows: one MCP server, disk-backed persistence, provider-agnostic search
  execution, bounded caches, and progressive disclosure via `retrieve()`.
- The next best improvements are workflow features, not another retrieval
  rewrite.
- Highest ROI:
  1. Add scoped rules and governance on top of existing `knowledge()`,
     permissions, and audit surfaces.
  2. Turn `SessionTracker` plus semantic memory into a stronger resume and boot
     flow.
  3. Make multi-repo and branch or task isolation easier to use without
     abandoning the unified local server.
- Keep the current token-efficiency strategy. The repo already shows strong
  baselines: `smart` chunking remains the best balanced default, `search`
  averages `336` output tokens in the current workflow benchmark, cache hit rate
  is `0.4615`, and TOON encoding reduces total output tokens by `17.2%`.

## Current Baseline

| Area | Current repo state | Evidence |
| --- | --- | --- |
| Unified architecture | Single MCP server with vector, BM25, graph, memory, git, and knowledge surfaces | `docs/ARCHITECTURE.md`, `src/contextia_mcp/server.py` |
| Search execution | Provider-agnostic shared runtime and search engine | `src/contextia_mcp/execution/runtime.py`, `src/contextia_mcp/execution/search.py` |
| Token shaping | Query-aware compaction, bookended previews, sandboxed full payloads | `src/contextia_mcp/execution/compaction.py`, `src/contextia_mcp/execution/response_policy.py`, `src/contextia_mcp/engines/output_sandbox.py` |
| Session continuity | Lightweight event tracker plus `session_snapshot()` | `src/contextia_mcp/memory/session_tracker.py`, `src/contextia_mcp/server.py` |
| Durable recall | LanceDB-backed semantic memory and `knowledge()` tool | `src/contextia_mcp/memory/memory_store.py`, `src/contextia_mcp/server.py` |
| Repo and branch context | Cross-repo registration, branch metadata, real-time branch watcher | `src/contextia_mcp/git/cross_repo.py`, `src/contextia_mcp/git/branch_watcher.py` |
| Governance primitives | Static tool permissions and structured audit logging already exist | `src/contextia_mcp/security/permissions.py`, `src/contextia_mcp/middleware/audit.py` |
| Retrieval benchmarks | `smart` chunking is the benchmark-backed default | `README.md`, `docs/DEVELOPER_GUIDE.md`, `scripts/benchmark_chunk_profiles.py` |

Current live defaults worth preserving unless benchmarks say otherwise:

- `CTX_CHUNK_CONTEXT_MODE=rich`
- `CTX_SMART_CHUNK_RELATIONSHIPS_ENABLED=true`
- `CTX_SMART_CHUNK_FILE_CONTEXT_ENABLED=true`
- `search_cache_max_size=128`, `search_cache_similarity_threshold=0.92`,
  `search_cache_ttl_seconds=300`
- `search_sandbox_threshold_tokens=1200`, `search_preview_results=4`,
  `search_preview_code_chars=220`

Current benchmark anchors:

- Chunking: `smart` is the best current default on the in-repo `src` /
  `20`-query benchmark at `MRR 0.625`, `Recall@5 0.9`, and `491` average tokens.
- Token efficiency: `336` tokens per `search`, `229` per `explain`,
  `0.4615` cache hit rate, `0.0` sandbox rate, and `17.2%` TOON output
  reduction in `scripts/token_benchmark_results.json`.
- Embeddings: among the retained benchmark set in
  `scripts/benchmark_results_full.json`, `bge-small-en` currently has the best
  measured `MRR@10` (`0.8115`) and `Recall@5` (`0.94`). That is useful evidence,
  but workflow ROI currently matters more than default-model churn.

## Source Matrix

| Source | Confirmed fact | Why it matters here | Confidence |
| --- | --- | --- | --- |
| [Anthropic, Contextual Retrieval](https://www.anthropic.com/engineering/contextual-retrieval) | Contextual chunk augmentation plus hybrid retrieval reduces top-k failures. | Reinforces `smart` chunking, contextual headers, and hybrid search rather than a pure-vector workflow. | High |
| [Liu et al., Lost in the Middle](https://arxiv.org/abs/2307.03172) | Long-context performance degrades when key evidence sits in the middle. | Supports query-aware compaction, short previews, and bookended result ordering in `execution/`. | High |
| [NVIDIA, RAG 101](https://developer.nvidia.com/blog/rag-101-retrieval-augmented-generation-questions-answered/) | Retrieval quality and latency depend on the whole pipeline, not one stage. | Validates benchmark-driven tuning across chunking, retrieval, reranking, and output shaping. | High |
| [NVIDIA, chunking strategy evaluation](https://developer.nvidia.com/blog/finding-the-best-chunking-strategy-for-accurate-ai-responses/) | Chunking strategy should be tested against the real corpus and query shape. | Justifies keeping `scripts/benchmark_chunk_profiles.py` as the decision gate for chunk defaults. | High |
| [OpenAI, Prompt Caching](https://openai.com/index/api-prompt-caching/) | Stable prefixes reduce repeated token cost and latency. | Supports resume packets, compact previews, and `retrieve()` over replaying long context. | High |
| [Windsurf, Memories & Rules](https://docs.windsurf.com/windsurf/cascade/memories) | Rules can be always-on, model-decision, glob, or manual; `AGENTS.md` becomes directory-scoped instructions. | Strong pattern for durable, scoped workflow guidance with lower context cost than long global prompts. | High |
| [Windsurf, Cascade Hooks](https://docs.windsurf.com/windsurf/cascade/hooks) | Pre and post hooks receive JSON on stdin, and pre-hooks can block actions with exit code `2`. | Maps directly onto governance, audit, and preflight policy opportunities around tool execution. | High |
| [Windsurf, Worktrees](https://docs.windsurf.com/windsurf/cascade/worktrees) | Parallel tasks run in separate worktrees, and `post_setup_worktree` bootstraps local state. | Useful pattern for optional task isolation, but should be adapted carefully to Contextia's path-mapping constraints. | Medium |
| [Devin, Declarative configuration](https://docs.devin.ai/onboard-devin/environment/blueprints) | Blueprints build snapshots; `knowledge` stores short lint, test, and build commands for session boot. | Strong model for repo-scoped workflow knowledge and fast session startup. | High |
| [Devin, Knowledge Onboarding](https://docs.devin.ai/onboard-devin/knowledge-onboarding) | Knowledge works best with specific triggers and specialized files such as `AGENTS.md`, `.windsurf`, and rule files. | Suggests Contextia should prefer typed, scoped workflow knowledge over large generic notes. | High |
| [Google, Gemini CLI README](https://raw.githubusercontent.com/google-gemini/gemini-cli/main/README.md) | Gemini CLI emphasizes checkpointing, token caching, custom context files, MCP support, and a terminal-first agent UX. | Reinforces stronger resume artifacts and repo-scoped context files without changing Contextia's local-first design. | Medium |

## Adopt, Adapt, Avoid

| Decision | Pattern | Why | Implementation targets |
| --- | --- | --- | --- |
| Adopt | Scoped durable instructions | Repo and path-scoped rules outperform large always-on prompts for workflow guidance. | `src/contextia_mcp/server.py`, `src/contextia_mcp/security/permissions.py`, `src/contextia_mcp/middleware/audit.py` |
| Adopt | Hook-based governance and observability | Contextia already has permission categories and audit logging; hook-like preflight checks are a natural extension. | `src/contextia_mcp/security/permissions.py`, `src/contextia_mcp/middleware/audit.py`, `src/contextia_mcp/server.py` |
| Adopt | Resume and checkpoint packets | Existing `SessionTracker`, semantic memory, and sandbox references already cover most primitives. | `src/contextia_mcp/memory/session_tracker.py`, `src/contextia_mcp/memory/memory_store.py`, `src/contextia_mcp/execution/runtime.py`, `src/contextia_mcp/server.py` |
| Adapt | Worktree or task isolation | Useful for parallel work, but should stay optional and local-first instead of becoming a required execution model. | `src/contextia_mcp/git/cross_repo.py`, `src/contextia_mcp/git/branch_watcher.py`, `src/contextia_mcp/server.py` |
| Adapt | Knowledge as command references | Keep workflow knowledge short, executable, and repo-scoped instead of turning it into a second documentation system. | `src/contextia_mcp/server.py`, `src/contextia_mcp/memory/memory_store.py`, `src/contextia_mcp/git/cross_repo.py` |
| Adapt | Large-context product claims | Use long context opportunistically, but keep retrieval selective and evidence near the top of the prompt. | `src/contextia_mcp/execution/compaction.py`, `src/contextia_mcp/execution/response_policy.py` |
| Avoid | Cloud-only or remote-index-first workflow features | They conflict with Contextia's privacy, portability, and low-memory positioning. | N/A |
| Avoid | Unbounded always-on rules or memories | They increase token cost, staleness, and debugging difficulty. | N/A |
| Avoid | Provider-specific execution coupling | The current provider-agnostic runtime is a strength and keeps benchmarking honest. | `src/contextia_mcp/execution/runtime.py`, `src/contextia_mcp/execution/search.py` |

## Ranked ROI Recommendations

### 1. Build scoped rules and workflow governance first

- Fact: Windsurf supports scoped rules, `AGENTS.md`, and blocking pre-hooks;
  Devin automatically reuses specialized instruction files.
- Inference: Contextia can add a thin rules layer on top of `knowledge()`,
  `permissions.py`, and `audit.py` without touching retrieval engines.
- Hypothesis: Repo or path-scoped rules plus fired-rule telemetry will cut
  repeated prompting and improve trust more than additional reranking work.
- Targets: `src/contextia_mcp/server.py`,
  `src/contextia_mcp/security/permissions.py`,
  `src/contextia_mcp/middleware/audit.py`.
- Suggested shape: store rules as lightweight knowledge entries or discovered
  repo files, attach scope metadata, log which rules were active for a tool
  call, and expose them in `session_snapshot()` or `introspect()`.

### 2. Turn session continuity into a first-class resume artifact

- Fact: Gemini CLI emphasizes checkpointing and token caching; OpenAI prompt
  caching rewards stable prefixes; Contextia already has `SessionTracker`,
  `OutputSandbox`, `retrieve()`, and semantic memory.
- Inference: The cheapest workflow gain is to return a deterministic resume
  packet instead of replaying large conversational context.
- Hypothesis: Adding recent search intents, active repo or branch, key actions,
  sandbox refs, and top relevant memories to `session_snapshot()` will improve
  long-running task continuity while lowering token cost.
- Targets: `src/contextia_mcp/memory/session_tracker.py`,
  `src/contextia_mcp/memory/memory_store.py`,
  `src/contextia_mcp/execution/runtime.py`, `src/contextia_mcp/server.py`.

### 3. Treat workflow knowledge as executable metadata, not free-form notes

- Fact: Devin blueprints keep `knowledge` entries short and executable, and
  knowledge retrieval works better with specific triggers.
- Inference: Contextia's `knowledge()` tool should evolve toward typed,
  repo-scoped workflow entries such as `lint`, `test`, `build`, `deploy`, and
  `style`.
- Hypothesis: Short executable workflow snippets will outperform long prose
  notes in search precision and agent behavior consistency.
- Targets: `src/contextia_mcp/server.py`,
  `src/contextia_mcp/memory/memory_store.py`,
  `src/contextia_mcp/git/cross_repo.py`.

### 4. Make parallel branch work easier without changing the core architecture

- Fact: Windsurf uses per-task worktrees plus a `post_setup_worktree` bootstrap
  hook; Contextia already tracks repo branch or head metadata and watches branch
  switches.
- Inference: Contextia should adapt the isolation idea through optional repo or
  task helpers instead of making worktrees a hard dependency of search.
- Hypothesis: Explicit task-isolation metadata and bootstrap hooks will improve
  multi-repo workflows without breaking host or container path mapping.
- Targets: `src/contextia_mcp/git/cross_repo.py`,
  `src/contextia_mcp/git/branch_watcher.py`, `src/contextia_mcp/server.py`.
- Guardrail: prefer explicit opt-in behavior; avoid automatic worktree creation
  in the core path until Docker and path-mapping edge cases are benchmarked.

### 5. Keep token-efficiency work benchmark-driven

- Fact: Anthropic contextual retrieval, NVIDIA's pipeline guidance, Liu et al.,
  and OpenAI caching all point to selective retrieval rather than brute-force
  context.
- Inference: Contextia's current `execution/compaction.py` plus
  `response_policy.py` direction is correct.
- Hypothesis: The next tuning win is likely threshold calibration and better
  resume packets, not larger default payloads. The current `sandbox_rate=0.0`
  suggests typical workloads do not yet cross the sandbox threshold often.
- Targets: `src/contextia_mcp/execution/compaction.py`,
  `src/contextia_mcp/execution/response_policy.py`,
  `src/contextia_mcp/engines/output_sandbox.py`,
  `scripts/benchmark_token_efficiency.py`.

## Risks and Tradeoffs

- Scoped rules can conflict or go stale. They need priority, dedupe, and
  visibility.
- Hook or preflight systems can slow responses or deadlock workflows if they
  block too aggressively.
- Worktree or task isolation complicates path mapping, Docker mounts, and
  untracked files.
- More durable memory increases the chance of recalling outdated instructions
  unless TTL, provenance, and pruning are surfaced clearly.

## Experiments

1. Rules experiment: add five repo or path-scoped rules, then track token
   overhead, fired-rule count, and user prompt repetition.
2. Resume experiment: compare baseline `session_snapshot()` against an enriched
   packet on multi-hour tasks; measure follow-up prompt length and recovery
   accuracy.
3. Workflow-knowledge experiment: seed typed `lint`, `test`, and `build`
   entries via `knowledge()`, then measure tool-selection speed and consistency.
4. Task-isolation experiment: register two repos plus a branch-switch workflow,
   then measure reindex freshness, path-mapping friction, and branch-watcher
   correctness.
5. Token experiment: vary sandbox threshold and preview sizes, rerun
   `scripts/benchmark_token_efficiency.py`, and verify retrieval quality does
   not regress.

## Final Recommendation

- Build the next layer around workflow control, not search novelty.
- The best sequence is:
  1. Scoped rules plus governance telemetry.
  2. Stronger resume packets plus relevant memory recall.
  3. Typed repo-scoped workflow knowledge.
- Revisit deeper worktree automation or model-default changes only after those
  three ship and are benchmarked.

## Primary Sources

1. Anthropic, "Contextual Retrieval" (2024):
   `https://www.anthropic.com/engineering/contextual-retrieval`
2. Liu et al., "Lost in the Middle" (2023):
   `https://arxiv.org/abs/2307.03172`
3. NVIDIA, "RAG 101: Retrieval-Augmented Generation Questions Answered" (2023):
   `https://developer.nvidia.com/blog/rag-101-retrieval-augmented-generation-questions-answered/`
4. NVIDIA, "Finding the Best Chunking Strategy for Accurate AI Responses"
   (2025):
   `https://developer.nvidia.com/blog/finding-the-best-chunking-strategy-for-accurate-ai-responses/`
5. OpenAI, "Prompt Caching in the API" (2024):
   `https://openai.com/index/api-prompt-caching/`
6. Windsurf, "Memories & Rules" (accessed 2026-05-06):
   `https://docs.windsurf.com/windsurf/cascade/memories`
7. Windsurf, "Cascade Hooks" (accessed 2026-05-06):
   `https://docs.windsurf.com/windsurf/cascade/hooks`
8. Windsurf, "Worktrees" (accessed 2026-05-06):
   `https://docs.windsurf.com/windsurf/cascade/worktrees`
9. Windsurf, "AGENTS.md" (accessed 2026-05-06):
   `https://docs.windsurf.com/windsurf/cascade/agents-md`
10. Devin, "Declarative configuration" (accessed 2026-05-06):
    `https://docs.devin.ai/onboard-devin/environment/blueprints`
11. Devin, "Knowledge Onboarding" (accessed 2026-05-06):
    `https://docs.devin.ai/onboard-devin/knowledge-onboarding`
12. Google, `gemini-cli` README (accessed 2026-05-06):
    `https://raw.githubusercontent.com/google-gemini/gemini-cli/main/README.md`
