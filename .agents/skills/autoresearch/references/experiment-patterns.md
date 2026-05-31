# Experiment Patterns

Use these patterns to keep Contextro experiment loops discriminating and comparable.

## Pick The Smallest Honest Harness

| Goal | Preferred harness | Why |
|---|---|---|
| cold index, search latency, throughput | `contextro-bench` | Measures end-to-end hot-path behavior and shipping thresholds |
| total tokens at fixed success | `contextro-study` | Captures token efficiency plus task success, not just one proxy |
| indexing pipeline changes | `cargo test -p contextro-indexing --test bench_index -- --nocapture` | Fast localized signal before rerunning broader bench/study |
| pre-release product safety | `./scripts/release-candidate.sh --skip-study` | Exercises real wrapper, persistence, and external-repo workflows |

## Good Contextro Experiment Shapes

1. **Compact response contract**
   - Hypothesis: a more compact payload keeps the same task success with fewer tokens.
   - Primary metric: lower `contextro-study` total tokens.
   - Guardrails: unchanged success, unchanged correctness, no missing fields the task depends on.

2. **Hot-path latency improvement**
   - Hypothesis: a cache, ranking shortcut, or data-structure change reduces latency.
   - Primary metric: lower `contextro-bench` search or cold-index latency.
   - Guardrails: same or better ranking quality, no throughput regression, no stale results.

3. **Exactness or routing improvement**
   - Hypothesis: better tool routing or contract guidance reduces wrong-tool calls and tokens.
   - Primary metric: lower study tokens or fewer tool calls at unchanged success.
   - Guardrails: no hidden work, no truthfulness regression, no benchmark harness changes.

## Keep / Discard Examples

- **Keep**: `contextro-study` total tokens fall materially, success stays `100%`, and task outputs
  stay equally informative.
- **Discard**: average search latency falls slightly but the delta is within run noise.
- **Discard**: a response gets shorter but omits the caller list or exact file path needed by the
  task.
- **Keep with caution**: a localized indexing improvement wins `bench_index`, then also holds on
  `contextro-bench`.

## Logging Discipline

For each experiment, record:

- hypothesis
- files changed
- exact command
- baseline
- measured result
- keep or discard
- what was learned

If the result is ambiguous, say so and rerun rather than narrating certainty.
