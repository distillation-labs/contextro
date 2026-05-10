"""Deterministic robustness analysis over paired-study task subsets.

This script reads the paired-study `results.json` emitted by
`scripts/experiment_platform.py`, keeps only tasks with a valid control-arm
equivalent, then draws reproducible bootstrap task subsets to estimate how
stable the token and latency gains remain across task mixtures.
"""

from __future__ import annotations

import argparse
import json
import random
import statistics
from pathlib import Path
from typing import Any


def _percentile(values: list[float], pct: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    if len(ordered) == 1:
        return round(float(ordered[0]), 2)
    rank = pct * (len(ordered) - 1)
    lower = int(rank)
    upper = min(lower + 1, len(ordered) - 1)
    weight = rank - lower
    value = ordered[lower] * (1 - weight) + ordered[upper] * weight
    return round(float(value), 2)


def _load_pairs(results_path: Path) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    rows = json.loads(results_path.read_text())
    grouped: dict[str, dict[str, dict[str, Any]]] = {}
    for row in rows:
        grouped.setdefault(row["task_id"], {})[row["arm"]] = row

    comparable: list[dict[str, Any]] = []
    excluded: list[dict[str, Any]] = []
    for task_id in sorted(grouped):
        pair = grouped[task_id]
        control = pair.get("control")
        mcp = pair.get("mcp")
        if control is None or mcp is None:
            raise ValueError(f"Missing arm for task {task_id}")
        if control.get("error") == "no_equivalent":
            excluded.append(
                {
                    "task_id": task_id,
                    "category": mcp.get("category", "unknown"),
                    "reason": "no_control_equivalent",
                }
            )
            continue
        comparable.append(
            {
                "task_id": task_id,
                "category": mcp.get("category", "unknown"),
                "control": control,
                "mcp": mcp,
            }
        )

    if not comparable:
        raise ValueError(f"No comparable tasks found in {results_path}")
    return comparable, excluded


def _subset_metrics(subset_id: int, sample: list[dict[str, Any]]) -> dict[str, Any]:
    control_tokens = sum(item["control"]["tokens_estimate"] for item in sample)
    mcp_tokens = sum(item["mcp"]["tokens_estimate"] for item in sample)
    control_latency = [item["control"]["wall_clock_ms"] for item in sample]
    mcp_latency = [item["mcp"]["wall_clock_ms"] for item in sample]

    return {
        "subset_id": subset_id,
        "task_ids": [item["task_id"] for item in sample],
        "tasks_sampled": len(sample),
        "unique_tasks": len({item["task_id"] for item in sample}),
        "control_tokens": control_tokens,
        "mcp_tokens": mcp_tokens,
        "token_reduction_pct": round((1 - mcp_tokens / max(control_tokens, 1)) * 100, 2),
        "control_median_latency_ms": round(statistics.median(control_latency), 2),
        "mcp_median_latency_ms": round(statistics.median(mcp_latency), 2),
        "control_mean_tool_calls": round(
            sum(item["control"]["tool_calls"] for item in sample) / len(sample), 2
        ),
        "mcp_mean_tool_calls": round(
            sum(item["mcp"]["tool_calls"] for item in sample) / len(sample), 2
        ),
        "control_mean_files_read": round(
            sum(item["control"]["files_read"] for item in sample) / len(sample), 2
        ),
        "mcp_mean_files_read": round(
            sum(item["mcp"]["files_read"] for item in sample) / len(sample), 2
        ),
    }


def _summary(subsets: list[dict[str, Any]]) -> dict[str, Any]:
    token_reduction = [item["token_reduction_pct"] for item in subsets]
    control_latency = [item["control_median_latency_ms"] for item in subsets]
    mcp_latency = [item["mcp_median_latency_ms"] for item in subsets]
    control_files = [item["control_mean_files_read"] for item in subsets]
    mcp_files = [item["mcp_mean_files_read"] for item in subsets]

    def block(values: list[float]) -> dict[str, Any]:
        return {
            "min": round(min(values), 2),
            "median": round(statistics.median(values), 2),
            "max": round(max(values), 2),
            "mean": round(statistics.mean(values), 2),
            "p2_5": _percentile(values, 0.025),
            "p97_5": _percentile(values, 0.975),
        }

    return {
        "token_reduction_pct": block(token_reduction),
        "control_median_latency_ms": block(control_latency),
        "mcp_median_latency_ms": block(mcp_latency),
        "control_mean_files_read": block(control_files),
        "mcp_mean_files_read": block(mcp_files),
    }


def main() -> dict[str, Any]:
    parser = argparse.ArgumentParser(
        description="Run deterministic robustness analysis over paired-study task subsets.",
    )
    parser.add_argument(
        "--results",
        type=Path,
        required=True,
        help="Path to paired-study results.json generated by experiment_platform.py",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Optional JSON output path",
    )
    parser.add_argument(
        "--subsets",
        type=int,
        default=100,
        help="Number of deterministic task subsets to generate",
    )
    parser.add_argument(
        "--sample-size",
        type=int,
        default=0,
        help="Tasks per subset; 0 means use all comparable tasks",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=20260509,
        help="Random seed for deterministic sampling",
    )
    parser.add_argument(
        "--label",
        default="paired-study-subset-robustness",
        help="Human-readable label embedded in the JSON output",
    )
    args = parser.parse_args()

    comparable, excluded = _load_pairs(args.results.resolve())
    sample_size = args.sample_size or len(comparable)
    if sample_size <= 0:
        raise ValueError("sample_size must be positive")

    rng = random.Random(args.seed)
    subsets = []
    for subset_id in range(1, args.subsets + 1):
        sample = [rng.choice(comparable) for _ in range(sample_size)]
        subsets.append(_subset_metrics(subset_id, sample))

    payload = {
        "label": args.label,
        "method": "bootstrap_task_subsets",
        "with_replacement": True,
        "seed": args.seed,
        "subsets": args.subsets,
        "sample_size": sample_size,
        "comparable_task_count": len(comparable),
        "comparable_task_ids": [item["task_id"] for item in comparable],
        "excluded_tasks": excluded,
        "summary": _summary(subsets),
        "subset_results": subsets,
    }

    rendered = json.dumps(payload, indent=2)
    print(rendered)
    if args.output is not None:
        args.output.write_text(rendered + "\n")
    return payload


if __name__ == "__main__":
    main()
