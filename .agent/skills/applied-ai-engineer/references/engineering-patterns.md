# Applied AI Engineering Patterns

This reference captures the repeated engineering patterns behind strong AI product teams.

## OpenAI

- The engineering role shifts from manual coding to systems, scaffolding, and leverage.
- Keep top-level instructions short and use the repository as the system of record.
- Enforce architecture and taste through tooling, not repeated review comments.
- Favor legibility, strict boundaries, and mechanically checked invariants.

## Anthropic

- Use the smallest high-signal context that still solves the task.
- Make long-running work resumable with explicit artifacts.
- Use progressive disclosure to avoid flooding the model with low-value detail.

## Cursor

- Keep the coding loop close to the local codebase: retrieve, inspect, edit, verify.
- Reduce friction between research, implementation, and validation so good ideas survive contact with the repo.
- Prefer precise context targeting over broad prompt stuffing.

## Windsurf

- Treat agent engineering as a coordinated plan-plus-execution system, not only a chat interaction.
- Keep visible intermediate state so long tasks can recover without losing intent.
- Make the environment, tools, and execution status legible enough for iterative autonomous work.

## Mistral

- Efficient model usage depends on good routing, compact context, and clean task decomposition.
- Smaller passes with strong structure can outperform a single large opaque pass.
- System quality comes from orchestration and interfaces, not only raw model size.

## Devin And Cognition

- Prefer realistic task environments over abstract unit-only evaluation.
- Use autonomous evaluator flows when deterministic checks are insufficient.
- Store external notes and environment state so long tasks can resume cleanly.

## NVIDIA

- Measure the entire RAG system: chunking, retrieval, reranking, shaping, latency, memory.
- The best architecture on paper is not the best architecture until it wins on the target corpus.

## DeepSeek

- Long-horizon systems benefit from checkpointing and resumable trajectories.
- Stable prompt structure improves reuse and efficiency.
- Preserve the useful state for tool-calling loops without replaying everything.

## What This Means For Contextro

- The next gains should come from harness quality, workflow control, observability, and resume flows.
- Research ideas should be translated into benchmarked, enforceable repo artifacts.
- New behavior should land with tests, evals, metrics, and a rollback story.
- Implementation patterns from Cursor, Windsurf, and Mistral should be translated into repo-local
  harnesses, workflow state, and efficient context/task routing rather than copied superficially.
