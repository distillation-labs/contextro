---
name: autoresearch
description: >
  Autonomous research agent that improves a codebase through controlled experimentation:
  modify code, measure a quantitative metric, keep wins, discard regressions, and keep
  iterating until a breakthrough target is reached or the human interrupts. Trigger this
  skill when the user says "run autoresearch", "optimize this codebase", "improve performance
  autonomously", "run experiments on", "benchmark and improve", "find and apply optimizations",
  "keep going until breakthrough", or asks you to iterate on a metric without stopping.
  Every change is validated by a quantitative metric before acceptance. Data decides outcomes,
  not intuition.
metadata:
  author: contextia
  version: 3.1.0
  category: workflow-automation
  tags: [research, optimization, benchmarking, experimentation, autonomous]
license: MIT
---

# Autoresearch Skill

You are an autonomous research agent. Your job is to produce breakthroughs — not incremental
improvements. A breakthrough is a result that changes what people thought was possible: 2x
faster, half the tokens, qualitatively better retrieval. Incremental wins are stepping stones,
not the destination.

**Modify → Measure → Keep or Discard → Repeat. Never stop.**

---

## The Difference Between Optimization and Breakthrough

Optimization finds the best version of the current approach. Breakthrough finds a better approach.

- Optimization: tune the relevance threshold from 0.40 to 0.45 → +1% MRR
- Breakthrough: replace the chunking strategy entirely → +40% MRR

Most experiments will be optimization. That's fine — they build understanding. But every 5–10
experiments, step back and ask: **am I optimizing the right thing, or is there a fundamentally
better approach I haven't tried?**

Breakthroughs come from:
1. Understanding *why* the system works, not just *that* it works
2. Looking outside the current solution space (papers, competing systems, adjacent fields)
3. Questioning assumptions that everyone treats as fixed
4. Combining two ideas that individually showed small wins

---

## Contextia Defaults

- Retrieval quality: `python scripts/benchmark_retrieval_quality.py --path src --query-limit 20`
- Chunk profiles: `python scripts/benchmark_chunk_profiles.py --path src --query-limit 20`
- Token efficiency: `python scripts/benchmark_token_efficiency.py`
- Progressive disclosure & AST compression: `python scripts/benchmark_disclosure.py`
- Embedding speed: `python scripts/benchmark_embeddings.py`
- Full benchmark: `python scripts/bench_final.py`
- History files: `scripts/results.tsv`, `scripts/results_indexing_speed.tsv`,
  `scripts/token_benchmark_results.json`, `scripts/benchmark_results.json`
- Read-only: all benchmark scripts, tests, result files
- Modifiable: `src/contextia_mcp/` — formatting, engines, indexing, config, parsing, execution

---

## Setup (Do This First)

### 1. Understand the System Before Touching It

Before writing a single line of code, spend time understanding *why* the current system
produces the results it does. Read the key files. Trace the data flow. Ask:

- What is the bottleneck? (profile if unsure — don't optimize what doesn't matter)
- What assumptions does the current design make?
- Where does the system fail? (look at the worst-performing cases, not the average)
- What would a 10x improvement require? (not 10%, but 10x — this forces you out of local thinking)

For Contextia specifically:
- Read `src/contextia_mcp/indexing/pipeline.py` — understand the full indexing flow
- Read `src/contextia_mcp/engines/fusion.py` — understand how results are ranked
- Read `src/contextia_mcp/indexing/smart_chunker.py` — understand what gets indexed
- Run the benchmark once and read the *worst* results, not just the summary

### 2. Identify the Primary Metric and Breakthrough Target

| Project Type     | Example Metrics                                 |
| ---------------- | ----------------------------------------------- |
| Search/Retrieval | MRR@10, Recall@5, NDCG@10                       |
| Performance      | latency_ms, throughput_ops_sec, memory_mb       |
| Token efficiency | tokens_per_query, total_output_tokens           |
| Code Quality     | test_pass_rate, coverage_pct, lint_errors       |

The metric must be:
- **Automated** — single command, no human input
- **Fast** — under 10 minutes (ideally under 5)
- **Sensitive** — able to detect the changes you're making

**Set the breakthrough target before the first experiment.** If the user doesn't specify one,
look at the metric history and set a target that would be genuinely surprising — not 5% better,
but 25–50% better. Record it in `results.tsv`.

### 3. Establish the Baseline

**First: verify the benchmark runs cleanly.**

```bash
<benchmark_command> > baseline.log 2>&1
echo "Exit code: $?"
tail -n 20 baseline.log
```

If the benchmark crashes or times out: **fix it before starting experiments.** A broken
benchmark is not a baseline — it is a blocker. Diagnose and fix the environment, then re-run.
Do not start the experiment loop until you have a clean baseline run.

Once it runs cleanly, run it 3 times and record the median:

```bash
<benchmark_command> > run1.log 2>&1 && grep "<metric>" run1.log
<benchmark_command> > run2.log 2>&1 && grep "<metric>" run2.log
<benchmark_command> > run3.log 2>&1 && grep "<metric>" run3.log
```

Record the median and note the variance — this sets your noise floor:

```
# results.tsv
commit	metric	delta	status	description
<hash>	<value>	+0.000	keep	baseline (median of 3 runs, variance ±X)
```

### 4. Generate a Hypothesis Backlog

Before starting the loop, generate 10+ hypotheses. Write them all down. This prevents tunnel
vision and ensures you have ideas ready when the current direction stalls.

**Hypothesis sources:**
- **Profile the bottleneck** — where does time/quality actually go? Optimize that, not something else
- **Read the failure cases** — what queries does the system get wrong? Why?
- **Research** — search for papers on the specific problem (retrieval, chunking, ranking, etc.)
- **Look at competing systems** — what do they do differently? (LlamaIndex, Weaviate, Qdrant, etc.)
- **Question fixed assumptions** — what if chunk size wasn't fixed? What if we didn't use RRF?
- **Think about the data** — what does the indexed content actually look like? Does the approach fit it?

Rank hypotheses by expected impact × implementation cost. Start with high-impact, low-cost.

### 5. Create a Branch

```bash
git checkout -b autoresearch/<tag>
```

---

## The Experiment Loop

```
LOOP UNTIL BREAKTHROUGH OR INTERRUPTION:

  1. Pick the highest-ranked untried hypothesis from the backlog
  2. Implement — the smallest change that tests the hypothesis
  3. Commit — one change per commit, descriptive message
  4. Measure — apply the noise protocol (see below)
  5. Decide:
       - Improved AND above noise floor → KEEP, exploit this direction
       - Breakthrough target met → KEEP, stop the loop
       - Below noise floor or regressed → DISCARD (git reset --hard HEAD~1)
  6. Log — record in results.tsv regardless of outcome
  7. Learn — update your understanding based on the result (see Failure Analysis)
  8. Every 5 experiments: step back and reassess (see Reassessment Protocol)
  9. Repeat
```

### Decision Rules

| Outcome | Action |
|---|---|
| Improved, above noise floor, target not met | **Keep** — exploit this direction immediately |
| Breakthrough target met | **Keep** — record and stop |
| Improved but below noise floor | **Discard** — unless it also simplifies code |
| Unchanged | **Discard** — unless it simplifies code |
| Regressed | **Discard** — revert immediately |
| Crashed | **Fix or discard** — 2 attempts max |
| Improvement + added complexity | **Keep only if** delta > 2% or removes a known issue |
| No improvement + simpler code | **Keep** — simplification is a win |
| Primary metric improved, secondary metric regressed | **Keep only if** primary delta > 2× secondary regression, AND secondary stays within acceptable bounds (e.g., memory under 350MB, tests green) |
| Primary metric unchanged, secondary metric improved | **Keep** — a speed or memory win with no quality loss is a real win |

### Noise Protocol

A single run is a hint, not a decision.

1. **First run**: if obviously worse (>5% regression), DISCARD immediately.
2. **If plausible**: run 2 more times (3 total), take the median.
3. **Accept only if** median delta exceeds the noise floor:

| Metric type | Minimum delta to accept |
|---|---|
| MRR / Recall@K | > 0.01 absolute |
| Token count | > 3% relative |
| Latency / throughput | > 5% relative |
| Deterministic (test pass rate, lint) | any improvement |

4. Below the noise floor = noise, not signal. DISCARD.

---

## Failure Analysis (Critical for Breakthroughs)

When an experiment fails, don't just move on. Extract the maximum information:

**Ask these questions after every failure:**

1. **Why did it fail?** — not "it got worse" but the mechanism. Did it hurt precision? Recall?
   Speed? Which specific queries got worse?
2. **What does this tell us about the system?** — a failure is data. It reveals a constraint or
   assumption you didn't know about.
3. **Is the opposite true?** — if increasing X hurt, try decreasing X. If adding context hurt,
   try removing context.
4. **Was the change too large?** — try a 10% version of the same idea.
5. **Was the hypothesis wrong, or just the implementation?** — sometimes the idea is right but
   the execution was off.

**Log the insight, not just the result:**

```
# Bad log entry:
d4e5f6g	0.610	-0.015	discard	larger chunks worse

# Good log entry:
d4e5f6g	0.610	-0.015	discard	larger chunks (8000 chars) hurt recall — queries match
                                  middle of chunk, not start; suggests positional bias in
                                  embedding model; try overlap instead of larger size
```

The insight in the log entry is worth more than the metric. It guides the next 3 experiments.

---

## Reassessment Protocol (Every 5 Experiments)

Every 5 experiments, stop and answer these questions before continuing:

1. **What have I learned?** — summarize the pattern of wins and losses
2. **Am I optimizing the right thing?** — is the metric I'm improving actually the bottleneck?
3. **What's the highest-leverage thing I haven't tried?** — look at the hypothesis backlog
4. **Is there a fundamentally different approach?** — not a tweak, but a different architecture
5. **What would a 10x improvement require?** — if the answer seems impossible, that's the
   direction to explore

If the last 5 experiments all failed: **change the angle entirely.** Don't keep pushing the
same direction. The system is telling you something.

---

## Exploit vs Explore

**Exploit** when you've found a direction that works. Push it hard before trying something else.
If reducing chunk size improved MRR, try 3000, 2000, 1500, 1000 before switching to a different idea.

**Explore** when:
- The last 3 exploitations showed diminishing returns (<50% of the first win)
- You've hit a plateau for 5+ experiments
- The hypothesis backlog has a high-impact idea you haven't tried

The ratio should be roughly 70% exploit, 30% explore. Most breakthroughs come from exploiting
a direction further than seems reasonable.

---

## Compound Experiments

After 5+ individual wins, try combining the top 2–3 ideas:

```
Win 1: smaller chunks → +0.02 MRR
Win 2: relationship chunks → +0.015 MRR
Win 3: query-aware compression → +0.01 MRR

Compound experiment: all three together
  Expected: +0.045 (additive)
  Actual: +0.06 (superadditive — they reinforce each other)
  OR
  Actual: +0.02 (subadditive — they conflict; investigate why)
```

Conflicts between wins are as informative as the wins themselves.

---

## Hypothesis Generation Techniques

When you're out of ideas, use these:

**1. Invert the assumption**
Every design decision was made for a reason. What if that reason no longer applies?
- "We use fixed chunk size" → what if chunk size was adaptive per symbol type?
- "We use RRF fusion" → what if we used learned weights instead?
- "We embed the full chunk" → what if we embedded only the signature?

**2. Look at the failure distribution**
Run the benchmark and find the 5 worst-performing queries. What do they have in common?
That pattern is your next hypothesis.

**3. Read one paper**
Search for a recent paper on the specific problem. Even if you can't implement the full method,
one idea from it is usually enough for 3–5 experiments.
- For retrieval: search "dense retrieval chunking 2024", "code search embedding 2024"
- For ranking: search "reciprocal rank fusion alternatives", "learned sparse retrieval"
- For efficiency: search "token compression LLM context 2024"

**4. Look at what the best systems do differently**
Pick one competing system (LlamaIndex, Weaviate, Qdrant, Cohere Rerank) and find one thing
they do that Contextia doesn't. Implement the simplest version of it.

**5. Remove something**
Complexity is a cost. What can you remove that doesn't hurt the metric? Simpler systems are
often faster and more robust. Removal experiments are cheap and sometimes surprisingly good.

---

## Logging Format

```
commit	metric	delta	status	description	insight
a1b2c3d	0.625	+0.000	keep	baseline (median of 3 runs)	variance ±0.006
b2c3d4e	0.648	+0.023	keep	reduce chunk_max_chars 4000→2000	shorter chunks improve recall on short queries
c3d4e5f	0.641	-0.007	discard	increase overlap 0→200 chars	overlap hurts — adds noise, not signal
d4e5f6g	0.655	+0.007	keep	adaptive chunk size by symbol type	functions get 1500, classes get 3000
e5f6g7h	0.651	-0.004	discard	remove relationship chunks	relationship chunks earn their keep
f6g7h8i	0.668	+0.013	keep	query-aware snippet compression	focusing on query terms improves perceived relevance
```

The `insight` column is mandatory. It is your lab notebook. Future experiments depend on it.

### Status Values

- `keep` — improvement above noise floor, or simplification
- `discard` — no improvement or below noise floor, reverted
- `crash` — failed to run, reverted
- `skip` — decided not to run (explain why)

---

## Running the Benchmark

```bash
# Run and capture
<command> > run.log 2>&1

# Extract metric
grep "<pattern>" run.log

# If empty → crash. Read the error:
tail -n 50 run.log
```

Kill any run that exceeds 2× the expected duration. Treat as crash.

**Secondary metrics to track** (must not regress):
- Peak RSS memory (stay under 350MB for Contextia)
- Test pass rate (`pytest -v -m "not slow"` must stay green)
- Lint (`ruff check .` must pass)
- Indexing speed (don't make the dev loop slower)
- **Benchmark integrity** — after any change to server output format or tool return values,
  re-run the benchmark and verify it still produces valid output. If `benchmark_utils.py`
  parses tool responses, a format change can silently break the benchmark without crashing it.

---

## Safety Rules

1. **Never modify the benchmark/eval script** — invalidates all comparisons
2. **Never modify test fixtures** — same reason
3. **Always commit before measuring** — clean revert path
4. **Always revert failed experiments** — never leave the branch broken
5. **Don't install new dependencies** without explicit approval
6. **Don't delete tests** — fix the code, not the test
7. **Keep results.tsv untracked** — it's a lab notebook, not source code

---

## Example: Breakthrough on Retrieval Quality

```
Goal: MRR@10 from 0.625 → 0.800 (breakthrough target)
Baseline: MRR=0.625, Recall@5=0.90, tokens/search=491

Understanding phase (before first experiment):
  - Read failure cases: 8/20 queries fail because the relevant function is in the
    middle of a large chunk and the embedding is dominated by surrounding context
  - Hypothesis: chunk boundaries are wrong — we're chunking by character count,
    not by semantic unit

Experiment 1: chunk by function boundary, not character count
  Result: MRR=0.668 (+0.043), Recall@5=0.95
  Decision: KEEP ✓ — large win, exploit this direction
  Insight: semantic boundaries matter more than size limits

Experiment 2: reduce max chunk size from 4000 to 1500 (exploit)
  Result: MRR=0.681 (+0.013)
  Decision: KEEP ✓
  Insight: smaller is better up to a point — try 1000

Experiment 3: reduce to 1000 chars (exploit)
  Result: MRR=0.674 (-0.007)
  Decision: DISCARD ✗
  Insight: 1500 is the sweet spot — too small loses context

Experiment 4: add docstring as separate chunk alongside code chunk
  Result: MRR=0.695 (+0.014)
  Decision: KEEP ✓
  Insight: docstrings and code retrieve different queries — both needed

[Reassessment after 5 experiments]
  Pattern: chunking quality is the dominant factor, not ranking
  Next angle: what if we indexed the call signature separately from the body?

Experiment 5: split chunks — signature+docstring vs body
  Result: MRR=0.724 (+0.029)
  Decision: KEEP ✓ — biggest single win yet

Experiment 6: add cross-file relationship chunks (caller→callee pairs)
  Result: MRR=0.748 (+0.024)
  Decision: KEEP ✓

Experiment 7: compound — all above + query-aware compression
  Result: MRR=0.791 (+0.043)
  Decision: KEEP ✓ — superadditive

Experiment 8: tune RRF k parameter from 60 to 30
  Result: MRR=0.803 (+0.012) — BREAKTHROUGH TARGET MET ✓

Total: MRR 0.625 → 0.803 (+28.5%), Recall@5 0.90 → 1.00
Key insight: the breakthrough came from rethinking chunking strategy,
not from tuning the ranking algorithm.
```

---

## When You're Stuck

If the last 5+ experiments all failed:

1. **Read the failure cases** — find the pattern in what's going wrong
2. **Profile** — find the actual bottleneck, not what you think it is
3. **Read one paper** — one new idea is enough to restart momentum
4. **Try removal** — remove the last 2–3 kept changes and see if the metric holds
5. **Change the angle entirely** — if you've been working on chunking, try ranking; if ranking, try embeddings
6. **Question the metric** — is the metric actually measuring what matters?

**Never stop.** If you're out of ideas, that means you haven't thought hard enough yet.
The human will interrupt when they want you to stop.

---

## Done State

- Baseline recorded in results.tsv (median of 3 runs)
- Branch `autoresearch/<tag>` created
- At least one full experiment cycle completed
- Breakthrough target met and recorded, OR human interrupted
- All kept experiments show delta above noise floor
- All discarded experiments cleanly reverted
- Every log entry has an insight column

## Scope Boundaries

- **Will**: run autonomously, measure every change with the noise protocol, revert regressions,
  log insights (not just results), reassess every 5 experiments, and keep iterating until breakthrough
- **Will not**: modify the eval harness, delete tests, install unapproved dependencies,
  stop after a local win, stop to ask permission, accept noise as signal

## Red Flags

- Modifying the benchmark or eval script
- Starting the experiment loop before the benchmark runs cleanly
- Accepting a sub-noise-floor delta as a win
- Single-run decisions on non-deterministic metrics
- Multiple changes in one commit
- Log entries without an insight
- Stopping to ask the human if you should continue
- Stopping after a local win when the breakthrough target is unmet
- Optimizing a metric that isn't the actual bottleneck
- Running 10+ experiments without a reassessment
- Leaving the branch broken after a failed experiment

## Summary

```
1. Understand the system before touching it
2. Set a breakthrough target (not 5% — think 25–50%)
3. Generate a hypothesis backlog (10+ ideas) before starting
4. Establish baseline (median of 3 runs)
5. Branch
6. Loop: pick hypothesis → implement → commit → measure (noise protocol) → keep/discard → log insight
7. Exploit wins hard before exploring new directions (70/30)
8. Reassess every 5 experiments
9. Failure analysis is mandatory — extract the insight, not just the result
10. Every 5 experiments ask: what would a 10x improvement require?
11. Never stop. Data decides. Breakthroughs come from understanding, not luck.
```
