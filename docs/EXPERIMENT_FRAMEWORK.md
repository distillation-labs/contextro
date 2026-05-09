# Contextro MCP Controlled Experiment Framework

## Overview

A two-arm controlled experiment comparing AI coding agent performance **with Contextro MCP** vs **without MCP** (baseline), using identical codebases and task sets.

## Experiment Arms

| Arm | Configuration | Description |
|-----|--------------|-------------|
| **Control** (no-MCP) | Agent uses only native tools (file read, grep, terminal) | Baseline performance without any MCP server |
| **Treatment** (MCP) | Agent uses Contextro MCP tools (search, find_symbol, explain, impact, etc.) | Full MCP-augmented workflow |

## Metrics

### Primary Metrics

| Metric | Unit | Collection Method |
|--------|------|-------------------|
| **Task completion rate** | % | Automated eval: does output match expected? |
| **Total tokens consumed** | count | Sum of input + output tokens across all tool calls |
| **Wall-clock time** | seconds | `perf_counter()` start-to-finish |
| **Tool calls count** | count | Number of tool invocations to complete task |

### Secondary Metrics

| Metric | Unit | Collection Method |
|--------|------|-------------------|
| **Files read** | count | Intercept file-read calls |
| **Correctness score** | 0–1 | LLM-as-judge or deterministic eval |
| **First-correct-attempt rate** | % | Did the agent get it right without retries? |
| **Context window utilization** | % | tokens_used / context_window_size |

### Guardrail Metrics

| Metric | Threshold | Purpose |
|--------|-----------|---------|
| Latency per tool call | <500ms p95 | Ensure MCP doesn't add unacceptable overhead |
| Memory usage | <500MB | Ensure MCP stays within resource bounds |
| Error rate | <5% | MCP tool failures shouldn't degrade experience |

## Task Set

Tasks are drawn from real coding workflows on the target codebase:

### Category 1: Symbol Discovery
- "Find the definition of `[FILL: symbol_name]`"
- "What functions call `[FILL: symbol_name]`?"
- "List all classes in `[FILL: directory]`"

### Category 2: Code Understanding
- "Explain how `[FILL: feature]` works"
- "What would break if I renamed `[FILL: symbol]`?"
- "Trace the data flow from `[FILL: entry_point]` to `[FILL: output]`"

### Category 3: Code Modification
- "Add error handling to `[FILL: function]`"
- "Refactor `[FILL: class]` to use dependency injection"
- "Fix the bug where `[FILL: symptom]`"

### Category 4: Cross-file Navigation
- "Find all usages of `[FILL: pattern]` across the codebase"
- "Which files import `[FILL: module]`?"
- "What's the dependency chain from `[FILL: A]` to `[FILL: B]`?"

## Data Collection

```
experiment_run/
├── config.json          # Arm, model, codebase, timestamp
├── tasks/
│   ├── task_001.json    # Task definition
│   ├── task_001_result.json  # Outcome + metrics
│   └── ...
├── summary.json         # Aggregated metrics
└── raw_logs/            # Full agent transcripts
```

### Per-task result schema

```json
{
  "task_id": "task_001",
  "arm": "mcp" | "control",
  "model": "claude-sonnet-4-20250514",
  "codebase": "/path/to/repo",
  "metrics": {
    "completed": true,
    "correctness_score": 0.95,
    "wall_clock_seconds": 12.3,
    "total_tokens": 4521,
    "tool_calls": 3,
    "files_read": 1,
    "retries": 0
  },
  "tool_trace": [
    {"tool": "search", "latency_ms": 4.2, "tokens_out": 116},
    {"tool": "explain", "latency_ms": 27.1, "tokens_out": 43}
  ]
}
```

## Statistical Approach

1. **Paired comparison**: Each task is run in both arms on the same codebase, same model, same prompt.
2. **Sample size**: Minimum 20 tasks per category (80+ total) for statistical power.
3. **Analysis**:
   - Wilcoxon signed-rank test for paired non-parametric comparisons
   - Bootstrap confidence intervals (95%) for median differences
   - Effect size: median ratio (MCP / control) for each metric
4. **Multiple comparisons**: Bonferroni correction across primary metrics.
5. **Reproducibility**: Fixed random seeds, pinned model versions, deterministic task ordering.

## Execution Protocol

```
1. Select target codebase (must be >1000 files for meaningful comparison)
2. Index codebase with Contextro (treatment arm only)
3. For each task in randomized order:
   a. Run in control arm → record metrics
   b. Run in treatment arm → record metrics
   c. Evaluate correctness (automated + spot-check)
4. Aggregate results
5. Compute statistical tests
6. Generate report
```

## Expected Outcomes (Hypotheses)

| Metric | Hypothesis | Based On |
|--------|-----------|----------|
| Tokens consumed | MCP arm uses 40–60% fewer tokens | evaluation.md: 116 tokens/search vs ~5000 tokens/file-read |
| Wall-clock time | MCP arm is 2–5x faster for discovery tasks | <2ms search vs sequential file reads |
| Files read | MCP arm reads 70–90% fewer files | Targeted retrieval vs exhaustive scanning |
| Correctness | MCP arm ≥ control (no degradation) | Better context → better answers |

## Integration with Skills Library

The experiment runner is distributed as part of the `@contextro/skills` npx package:

```bash
npx @contextro/skills benchmark --codebase /path/to/repo --tasks ./tasks.json
```

This allows customers to reproduce the experiment on their own codebases.
