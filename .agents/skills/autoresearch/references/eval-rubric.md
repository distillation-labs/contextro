# Eval Rubric

Score the skill on whether it keeps Contextro experiment loops disciplined and benchmark-honest.

## Required Behaviors

- Establishes a real baseline before edits.
- Chooses the benchmark that actually measures the requested outcome.
- Names a breakthrough target, not just a vague improvement goal.
- Runs one-variable experiments by default.
- Keeps or discards changes using measured results, not vibes.
- Protects success, truthfulness, and release guardrails while chasing speed or token wins.

## Strong Answer Signals

- Cites `contextro-bench`, `contextro-study`, `bench_index`, or the RC workflow correctly.
- Uses retained benchmark evidence from the repo instead of invented numbers.
- Treats smaller but less useful outputs as regressions.
- Calls out noise discipline when the metric is noisy.
- Changes angle after repeated failed experiments.

## Failure Signals

- Starts changing code before measuring the baseline.
- Recommends editing the benchmark harness to improve the score.
- Accepts token or latency wins that lower success or honesty.
- Bundles multiple untracked variables into the first experiment.
- Stops after a tiny local win with no breakthrough target or next experiment.
