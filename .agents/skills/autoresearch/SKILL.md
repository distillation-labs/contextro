---
name: autoresearch
description: Use for autonomous, metric-driven experiment loops on Contextro. Trigger when the user asks to benchmark and improve latency, token efficiency, retrieval quality, or tool robustness until a breakthrough target is met, with keep-or-discard decisions based on real measurements.
when_to_use: Especially useful for `contextro-bench`, `contextro-study`, indexing, response-contract, or release-gate optimization loops where the agent should measure, change one variable at a time, keep only wins, and continue without pausing after each experiment.
metadata:
  version: 1.0.0
  category: research-engineering
  tags:
    - contextro
    - benchmark
    - study
    - latency
    - token-efficiency
    - retrieval
    - experiment-loop
    - regression
    - mcp
license: Proprietary
---

# Autoresearch

Run a disciplined experiment loop against Contextro's real harnesses. Measure first, change one
thing at a time, keep only wins that beat noise and preserve truthfulness.

## Use This Skill To Produce

- a baseline grounded in the repo's real benchmark outputs
- a concrete breakthrough target
- a ranked hypothesis backlog
- one-variable experiments with clear attribution
- keep or discard decisions based on measured results
- a durable experiment log with results and insights

## Contextro Defaults

Primary benchmark and guardrail surfaces:

- `cd crates && cargo run --release -p contextro --bin contextro-bench -- <repo-path>`
- `cd crates && cargo run --release -p contextro --bin contextro-study -- --codebase <repo-path> --output-dir <dir> --tasks 200`
- `cd crates && cargo test --quiet`
- `cd crates && cargo test --quiet -p contextro-indexing --test bench_index -- --nocapture`
- `./scripts/release-candidate.sh --skip-study`

Historical result sources:

- `.agents/skills/dev-contextro-mcp/references/benchmark-results.md`
- `README.md`
- generated `contextro-study` outputs from prior runs or RC workspaces

Read-only by default:

- benchmark and study harnesses
- tests and fixed fixtures
- retained benchmark result references
- release-candidate harness scripts

Primary modifiable surfaces:

- `crates/contextro-indexing/`
- `crates/contextro-engines/`
- `crates/contextro-tools/`
- `crates/contextro-server/`
- `.agents/skills/` when the experiment is about agent performance or routing quality

## Repository File Size Rule

- Treat hand-written source-file size as a hard repository constraint during experiments.
- Keep every source file in the **300-500 line** band.
- Do **not** create a new source file outside that band without explicit user approval.
- Do **not** grow an existing source file past **500 lines**.
- If any touched hand-written source file is already over **500 lines**, the experiment must
  include the extraction or split in the same task.

## Method

### 1. Establish The Real Baseline First

Before changing code:

1. Read the existing retained result references.
2. Read the relevant benchmark script or harness and the target source files.
3. Verify the benchmark runs cleanly in the current environment.
4. Record the baseline and noise characteristics.

Do not start experiments until the baseline is reproducible.

### 2. Define The Metric And Breakthrough Target

Name:

- the primary metric
- the direction of improvement
- the secondary guardrails
- the breakthrough target

For Contextro, common primary metrics are:

- lower `contextro-study` total tokens at unchanged success
- lower `contextro-bench` cold-index latency
- lower `contextro-bench` average search latency
- higher `contextro-bench` MCP throughput
- lower restore or repo-scope latency without correctness loss

Default guardrails:

- keep `contextro-study` at `100%` success on the retained repo study
- preserve public tool correctness and output truthfulness
- do not meaningfully regress `contextro-bench`
- keep release-candidate behavior at least as good as the previous retained state

### 3. Build A Hypothesis Backlog

Generate several candidate ideas before starting the loop.

Source hypotheses from:

- benchmark failure cases
- profiling or bottleneck evidence
- current repo architecture
- retained benchmark notes
- outside research only when the mechanism transfers cleanly

Rank by expected impact, implementation cost, reversibility, and how fast the benchmark can
discriminate.

### 4. Run One-Variable Experiments

Each experiment should test one main idea.

Required sequence:

1. pick the next hypothesis
2. implement the smallest change that tests it
3. keep a clean revert path; prefer commit-level attribution when appropriate, otherwise keep the
   patch isolated and trivially revertible
4. run the benchmark
5. compare against baseline and guardrails
6. keep or discard
7. log the result and the insight

Do not bundle multiple variables unless you are intentionally running a later compound
experiment.

### 5. Use Noise Discipline

- For noisy metrics, rerun and compare the median against the noise floor.
- For deterministic metrics, a single reproducible improvement may be enough.
- Below-noise deltas are not wins.

### 6. Keep Only Real Wins

Keep a change only when:

- the primary metric improves enough to matter
- the guardrails stay green
- output contracts remain truthful and useful
- the benchmark itself remains comparable

Discard or revert when:

- the metric regresses
- the gain is within noise
- success falls, ranking quality flattens, or outputs become less honest
- tests fail and cannot be fixed quickly

### 7. Reassess Regularly

Every few experiments, step back and ask:

- what have we learned
- which direction is actually working
- whether we are optimizing the right bottleneck
- whether a different angle has higher leverage

If several experiments fail in a row, change angle rather than thrashing.

### 8. Compound Only After Isolated Wins

After several strong individual wins, combine them deliberately and call out whether the
interaction is additive, superadditive, or conflicting.

## Decision Rules

- Keep improvements that beat the noise floor and preserve guardrails.
- Keep simplifications when they do not hurt the primary metric.
- Revert regressions immediately.
- Revert or discard changes that make outputs smaller but less truthful.
- Do not stop after a local win if the breakthrough target is still unmet.
- Stop when the breakthrough target is met or the user interrupts.

## Safety Rules

- Never modify benchmark or eval scripts to improve the score.
- Never modify fixed fixtures to make results look better.
- Never delete tests to keep an experiment.
- Always keep a clean revert path.
- Do not install new dependencies without approval.
- Preserve benchmark comparability across the whole run.

## Output Format

Return work in this order:

1. `Metric and breakthrough target`
2. `Baseline and guardrails`
3. `Hypothesis backlog`
4. `Current experiment`
5. `Measured result`
6. `Keep or discard decision`
7. `Insight logged`
8. `Next experiment`

## Anti-Patterns

- starting experiments before the baseline is verified
- changing multiple variables at once by default
- using a giant cleanup rewrite as the first experiment
- accepting sub-noise-floor deltas as wins
- optimizing the score by changing the measuring stick
- taking a token win that lowers success or hides evidence the user still needs
- leaving the branch or working tree in a broken state after a failed run

## Handoff Rule

- use `breakthrough-researcher` when the solution space is still unclear
- use `applied-ai-engineer` when the best next step is hardening, rollout, or observability

## References

- `references/experiment-patterns.md`
- `references/eval-rubric.md`
