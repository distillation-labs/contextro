# Research Patterns

These patterns keep Contextro research grounded and experimentally useful.

## Start With The Current System

Before looking outward, answer:

- What does Contextro already measure?
- Which benchmark number is actually unsatisfactory?
- Which constraint is binding: latency, tokens, success, memory, contract truthfulness, or
  release stability?

If that is unclear, the research question is still too broad.

## Mechanism-First Synthesis

For each external idea, capture:

1. **mechanism** — what actually changed
2. **metric** — what improved
3. **transfer path** — where it fits in Contextro
4. **limits** — where the analogy breaks

Example:

- "Progressive disclosure" is a mechanism.
- "Anthropic does it" is not enough.

## High-Leverage Contextro Research Themes

1. **Compact, truthful response contracts**
   - How can outputs shrink without hiding exact names, files, lines, or risk?
2. **Hybrid retrieval routing**
   - Where should exact/BM25, graph, vector, and AST paths win by default?
3. **Hot-path latency**
   - Which index or ranking stages dominate wall time?
4. **Study integrity**
   - Which benchmark tasks catch over-compression or misleading summaries?
5. **Long-running workflow resilience**
   - How do compaction, recovery, repo scope, and release gating stay stable under iteration?

## Good Top-Experiment Shape

Each recommended experiment should be:

- narrow enough to falsify quickly
- tied to one main metric
- explicit about the expected gain
- explicit about the guardrails
- reversible

Poor experiment:

- "Improve search a lot"

Good experiment:

- "Add a lower-token exact-hit response path for unique symbol matches and rerun the retained
  200-task repo study; keep only if total tokens fall materially and success stays 100%."
