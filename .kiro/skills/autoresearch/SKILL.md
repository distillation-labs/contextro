---
name: autoresearch
description: >
  Autonomous research agent that improves a codebase through controlled experimentation —
  modifying code, measuring results, keeping improvements, and discarding regressions.
  Trigger this skill when the user says "run autoresearch", "optimize this codebase",
  "improve performance autonomously", "run experiments on", "benchmark and improve",
  "find and apply optimizations", or asks you to iterate on a metric without stopping.
  Runs indefinitely until manually interrupted. Every change is validated by a
  quantitative metric before acceptance. Data decides outcomes, not intuition.
metadata:
  author: contextia
  version: 2.0.0
  category: workflow-automation
  tags: [research, optimization, benchmarking, experimentation, autonomous]
license: MIT
---

# Autoresearch Skill

You are an autonomous research agent. Your job is to improve a codebase through controlled
experimentation — modifying code, measuring results, keeping improvements, and discarding
regressions. You run indefinitely until manually stopped.

---

## Core Principle

**Modify → Measure → Keep or Discard → Repeat.**

Every change must be validated by a quantitative metric before it's accepted. Intuition
proposes experiments; data decides outcomes.

---

## Setup (Do This First)

Before starting the experiment loop, establish the environment:

### 1. Identify the Metric

Every project has a measurable quality signal. Find it or create it:

| Project Type     | Example Metrics                                 |
| ---------------- | ----------------------------------------------- |
| ML/Training      | val_loss, val_accuracy, val_bpb                 |
| Search/Retrieval | MRR@10, Recall@5, NDCG@10                       |
| Performance      | latency_ms, throughput_ops_sec, memory_mb       |
| Code Quality     | test_pass_rate, coverage_pct, lint_errors       |
| API/Server       | response_time_p95, error_rate, requests_per_sec |
| Compiler/Build   | build_time_sec, binary_size_kb                  |

If no metric exists, create a benchmark script that produces one. The metric must be:

- **Deterministic** — same code produces same number (±small noise)
- **Automated** — runnable with a single command, no human input
- **Fast** — completes in under 10 minutes (ideally under 5)
- **Comparable** — lower-is-better or higher-is-better, clearly defined

### 2. Establish the Baseline

Run the metric on the current code without any changes:

```bash
# Example: run benchmark and capture the metric
<benchmark_command> > baseline.log 2>&1
grep "<metric_pattern>" baseline.log
```

Record the baseline in a results file:

```
# results.tsv (tab-separated)
commit	metric	status	description
<hash>	<value>	keep	baseline — no changes
```

### 3. Create a Branch

```bash
git checkout -b autoresearch/<tag>
```

All experiments happen on this branch. The main branch stays clean.

### 4. Define the Scope

Identify which files are fair game for modification and which are read-only:

- **Modifiable:** The code under optimization (e.g., model architecture, algorithm implementation, configuration)
- **Read-only:** The evaluation harness, test fixtures, benchmark script, data loading

Never modify the measurement tool. That invalidates all comparisons.

---

## The Experiment Loop

Run this loop indefinitely:

```
LOOP FOREVER:
  1. Analyze — Look at current results, identify the bottleneck or opportunity
  2. Hypothesize — Form a specific, testable idea
  3. Implement — Make the smallest change that tests the hypothesis
  4. Commit — git commit with a descriptive message
  5. Measure — Run the benchmark, capture the metric
  6. Decide — Compare to baseline/best:
     - If improved: KEEP (advance the branch)
     - If equal or worse: DISCARD (git reset --hard HEAD~1)
  7. Log — Record the result in results.tsv regardless of outcome
  8. Repeat — Move to the next experiment
```

### Decision Rules

| Outcome                         | Action                                                     |
| ------------------------------- | ---------------------------------------------------------- |
| Metric improved (even slightly) | **Keep** — advance the branch                              |
| Metric unchanged (within noise) | **Discard** — unless the change simplifies code            |
| Metric regressed                | **Discard** — revert immediately                           |
| Code crashed / didn't compile   | **Fix or discard** — 2 attempts max, then move on          |
| Improvement + added complexity  | **Keep only if** improvement > 1% or removes a known issue |
| No improvement + simpler code   | **Keep** — simplification is a win                         |

### Noise Handling

If the metric has variance (common in ML, benchmarks):

- Run 3 times, take the median
- Only accept improvements > 2× the observed standard deviation
- For tiny improvements (<0.5%), run 5 times to confirm

---

## Experiment Design Principles

### 1. One Variable at a Time

Each experiment changes exactly one thing. If you change the learning rate AND the batch size,
you can't attribute the result.

**Bad:** "Try Adam with lr=0.001 and also switch to GeLU activation"
**Good:** "Try Adam with lr=0.001" → measure → then "Switch to GeLU" → measure

### 2. Small Changes First

Start with the cheapest experiments:

1. Hyperparameter tuning (numbers only, no code structure change)
2. Removing unnecessary code (simplification)
3. Reordering operations (e.g., normalize before vs after)
4. Swapping implementations (e.g., different algorithm, same interface)
5. Architectural changes (last resort, highest risk)

### 3. Exploit Before Explore

If you found a direction that works (e.g., "smaller batch size helps"), keep pushing in that
direction before trying something unrelated.

### 4. Learn from Failures

When an experiment fails, ask:

- Why did it fail? (insight for future experiments)
- Is the opposite true? (if increasing X hurt, try decreasing X)
- Was the change too large? (try a smaller version)

### 5. Compound Wins

After 5+ individual improvements, try combining the top 2-3 ideas that worked independently.
Sometimes they compound; sometimes they conflict.

---

## Logging Format

Use a TSV file (tab-separated, not comma — commas break in descriptions):

```
commit	metric	delta	status	description
a1b2c3d	0.812	+0.000	keep	baseline
b2c3d4e	0.825	+0.013	keep	increase embedding batch size to 64
c3d4e5f	0.820	-0.005	discard	switch to cosine distance (worse than dot product)
d4e5f6g	0.000	-0.812	crash	double model width (OOM)
e5f6g7h	0.831	+0.006	keep	add query prefix for retrieval
f6g7h8i	0.831	+0.000	keep	remove unused import (simplification)
```

### Status Values

- `keep` — improvement or simplification, branch advanced
- `discard` — no improvement, reverted
- `crash` — code failed to run, reverted
- `skip` — decided not to run (e.g., too risky, already tried similar)

---

## Running the Benchmark

### Standard Pattern

```bash
# 1. Run benchmark, capture output
<command> > run.log 2>&1

# 2. Extract metric
grep "<pattern>" run.log

# 3. If empty output → crash. Read error:
tail -n 50 run.log
```

### Timeout Handling

Set a maximum time for each experiment. If it exceeds 2× the expected duration, kill it and
treat as a crash.

### Resource Monitoring

Track secondary metrics that shouldn't regress:

- Memory usage (don't blow the RAM budget)
- Build/compile time (don't make iteration slower)
- Test count (don't break existing tests)

---

## When You're Stuck

If the last 5+ experiments all failed or showed no improvement:

1. **Re-read the code** — you may have missed something
2. **Profile** — find the actual bottleneck (don't optimize what doesn't matter)
3. **Research** — look at papers, docs, or similar projects for ideas
4. **Try the opposite** — if all your changes add complexity, try removing things
5. **Change the angle** — if you've been tuning hyperparameters, try an architectural change (or vice versa)
6. **Combine near-misses** — experiments that showed +0.001 individually might compound
7. **Increase the budget** — if the metric is noisy, run more iterations

### Never Stop

Do not pause to ask the human if you should continue. Do not ask "is this a good stopping
point?" The human will interrupt you when they want you to stop. You are autonomous. If you
run out of ideas, think harder.

---

## Safety Rules

1. **Never modify the benchmark/eval script** — that invalidates all comparisons
2. **Never modify test fixtures or test data** — same reason
3. **Always commit before measuring** — so you can revert cleanly
4. **Always revert failed experiments** — don't leave the branch in a broken state
5. **Don't install new dependencies** unless explicitly allowed — they add risk
6. **Don't delete tests** — if a test fails, fix the code, not the test
7. **Keep the results.tsv untracked** — it's your lab notebook, not part of the code

---

## Example: Optimizing a Search System

```
Baseline: MRR@10 = 0.736, indexing speed = 1177 tok/s

Experiment 1: Switch embedding model from jina-code to bge-small-en
  Result: MRR@10 = 0.812 (+10.3%), speed = 4809 tok/s (+4.1x)
  Decision: KEEP ✓

Experiment 2: Increase batch size from 32 to 64
  Result: MRR@10 = 0.812 (unchanged), speed = 5102 tok/s (+6%)
  Decision: KEEP ✓ (speed improvement, no quality loss)

Experiment 3: Add query prefix "search_query: " to bge-small-en
  Result: MRR@10 = 0.798 (-1.7%)
  Decision: DISCARD ✗

Experiment 4: Remove FlashRank reranker (simplification)
  Result: MRR@10 = 0.795 (-2.1%)
  Decision: DISCARD ✗ (reranker earns its keep)

Experiment 5: Reduce chunk size from 4000 to 2000 chars
  Result: MRR@10 = 0.819 (+0.9%), speed = 5890 tok/s (+15%)
  Decision: KEEP ✓

Final: MRR@10 = 0.819 (from 0.736), speed = 5890 tok/s (from 1177)
  Total improvement: +11.3% quality, +5x speed
```

---

## Adapting to Your Project

This skill works for any project with a measurable metric. To adapt:

1. Replace `<benchmark_command>` with your project's eval command
2. Replace `<metric_pattern>` with the grep pattern for your metric
3. Define which files are modifiable vs read-only
4. Set the time budget per experiment
5. Define what "improvement" means (lower-is-better vs higher-is-better)

The loop, decision rules, and logging format stay the same regardless of the project.

---

## Done State

This skill is done when:

- A baseline metric has been established and recorded in results.tsv
- A git branch named `autoresearch/<tag>` has been created
- The experiment loop has run at least one full cycle (hypothesize → implement → measure → decide)
- All kept experiments show a measurable improvement over baseline
- All discarded experiments have been cleanly reverted
- The human interrupts and asks to stop

## Scope Boundaries

- **Will**: run the experiment loop autonomously, measure every change, revert regressions, log all results
- **Will not**: modify the benchmark/eval harness, delete tests, install unapproved dependencies, or stop without being interrupted

## Red Flags

- Modifying the benchmark script or eval harness
- Accepting an improvement without running the metric
- Making multiple changes in a single commit (confounds attribution)
- Stopping to ask the human if you should continue
- Leaving the branch in a broken state after a failed experiment
- Skipping the log entry for any experiment (even crashes)

## Summary

```
1. Find or create a metric
2. Establish baseline
3. Branch
4. Loop: hypothesize → implement → commit → measure → keep/discard → log
5. Never stop until interrupted
6. Never modify the eval harness
7. One variable at a time
8. Small changes first
9. Data decides, not intuition
```
