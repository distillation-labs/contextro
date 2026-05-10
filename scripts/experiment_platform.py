"""Controlled paired experiment: Contextro MCP vs grep-plus-read discovery.

Runs identical tasks in two arms:
  - Control: grep + file reads (simulating agent without MCP)
  - Treatment: Contextro MCP tool calls (real server)

The archived run targeted a private production monorepo. This script now keeps the
task set fixed while allowing the codebase path, output directory, and storage
directory to be overridden for reproduction on a local checkout of that corpus or
an adapted equivalent repository.
"""

# ruff: noqa: E402

from __future__ import annotations

import argparse
import asyncio
import json
import os
import subprocess
import sys
import time
from dataclasses import asdict, dataclass
from pathlib import Path

# Add contextro source to path
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "src"))

from contextro_mcp.experiment_tasks import (
    COMPARABLE_TASK_CATALOG_FILENAME,
    FULL_TASK_CATALOG_FILENAME,
    TASKS,
    Task,
    comparable_tasks,
    mcp_native_tasks,
    write_task_catalogs,
)
from contextro_mcp.token_counting import count_serialized_tokens, tokenizer_metadata

DEFAULT_CODEBASE = os.environ.get("CTX_EXPERIMENT_CODEBASE", "")
DEFAULT_OUTPUT_DIR = Path(
    os.environ.get("CTX_EXPERIMENT_OUTPUT_DIR", str(ROOT / "scripts" / "experiment_results"))
)
COMPARABLE_TASK_COUNT = len(comparable_tasks())
MCP_NATIVE_TASK_COUNT = len(mcp_native_tasks())


@dataclass
class TaskResult:
    task_id: str
    arm: str  # "control" or "mcp"
    category: str
    completed: bool = False
    wall_clock_ms: float = 0.0
    tokens_estimate: int = 0
    tool_calls: int = 0
    files_read: int = 0
    results_count: int = 0
    error: str = ""


def run_control_arm(task: Task, codebase_path: str) -> TaskResult:
    """Simulate agent without MCP: grep for a pattern, then read matching files."""
    start = time.perf_counter()
    files_read = 0
    tokens = 0
    tool_calls = 0
    results_count = 0

    if not task.grep_pattern:
        elapsed = (time.perf_counter() - start) * 1000
        return TaskResult(
            task_id=task.id,
            arm="control",
            category=task.category,
            completed=True,
            wall_clock_ms=round(elapsed, 2),
            tokens_estimate=0,
            tool_calls=0,
            files_read=0,
            results_count=0,
            error="no_equivalent",
        )

    try:
        tool_calls += 1
        grep_cmd = [
            "grep",
            "-rl",
            "--include=*.ts",
            "--include=*.tsx",
            "--include=*.js",
            "--include=*.mjs",
            "--exclude-dir=node_modules",
            "--exclude-dir=.git",
            "--exclude-dir=_generated",
            "--exclude-dir=.next",
            task.grep_pattern,
            codebase_path,
        ]
        grep_result = subprocess.run(
            grep_cmd,
            capture_output=True,
            text=True,
            timeout=15,
            env={**os.environ, "LC_ALL": "C"},
        )
        matching_files = [f for f in grep_result.stdout.strip().split("\n") if f][:10]
        results_count = len(matching_files)
        tokens += count_serialized_tokens(grep_result.stdout)

        for fpath in matching_files[:5]:
            tool_calls += 1
            files_read += 1
            try:
                content = Path(fpath).read_text(errors="ignore")
            except (OSError, PermissionError):
                continue
            tokens += count_serialized_tokens(content)

    except subprocess.TimeoutExpired:
        tokens = 3000
        files_read = 3
        tool_calls = 4
    except Exception as exc:
        return TaskResult(
            task_id=task.id,
            arm="control",
            category=task.category,
            error=str(exc),
        )

    elapsed = (time.perf_counter() - start) * 1000
    return TaskResult(
        task_id=task.id,
        arm="control",
        category=task.category,
        completed=True,
        wall_clock_ms=round(elapsed, 2),
        tokens_estimate=tokens,
        tool_calls=tool_calls,
        files_read=files_read,
        results_count=results_count,
    )


def _configure_environment(storage_dir: str, embedding_model: str) -> None:
    """Set environment for the isolated treatment arm runtime."""
    os.environ["CTX_STORAGE_DIR"] = storage_dir
    os.environ["CTX_LOG_LEVEL"] = "WARNING"
    os.environ["CTX_EMBEDDING_MODEL"] = embedding_model


def _reset_runtime():
    """Reset and create a fresh MCP server instance."""
    import contextro_mcp.server as server_module
    from contextro_mcp.config import reset_settings
    from contextro_mcp.state import reset_state

    reset_settings()
    reset_state()
    server_module._pipeline = None
    server_module._index_job = {}
    return server_module.create_server(), server_module


async def index_and_wait(mcp, server_module, path: str, timeout: int = 300) -> dict:
    """Index the codebase and wait for completion."""
    from benchmark_utils import call_tool

    result = await call_tool(mcp, "index", {"path": path})
    if result.get("status") != "indexing":
        return result

    for _ in range(timeout * 2):
        await asyncio.sleep(0.5)
        with server_module._index_job_lock:
            status = server_module._index_job.get("status")
            if status == "done":
                return server_module._index_job.get("result", {})
            if status == "error":
                return {"error": server_module._index_job.get("error", "unknown")}

    return {"error": "timeout"}


async def run_mcp_arm(task: Task, mcp) -> TaskResult:
    """Run a task using Contextro MCP tools."""
    from benchmark_utils import call_tool, estimate_tokens

    start = time.perf_counter()
    result = await call_tool(mcp, task.mcp_tool, task.mcp_args)
    elapsed = (time.perf_counter() - start) * 1000

    has_error = "error" in result and result["error"]
    tokens = estimate_tokens(result)
    results_count = 0

    if isinstance(result, dict):
        for key in ("callers", "callees", "results", "affected", "direct_callers"):
            if key in result and isinstance(result[key], list):
                results_count = len(result[key])
                break

    return TaskResult(
        task_id=task.id,
        arm="mcp",
        category=task.category,
        completed=not has_error,
        wall_clock_ms=round(elapsed, 2),
        tokens_estimate=tokens,
        tool_calls=1,
        files_read=0,
        results_count=results_count,
        error=str(result.get("error", "")) if has_error else "",
    )


def analyze_results(results: list[TaskResult]) -> dict:
    """Compute summary statistics for both experiment arms."""
    control = [result for result in results if result.arm == "control"]
    mcp = [result for result in results if result.arm == "mcp"]
    comparable_task_ids = {task.id for task in comparable_tasks()}

    def stats(items: list[TaskResult], key: str) -> dict:
        values = [getattr(result, key) for result in items if result.completed]
        if not values:
            return {"count": 0, "sum": 0, "mean": 0, "median": 0, "p95": 0}
        values.sort()
        count = len(values)
        return {
            "count": count,
            "sum": sum(values),
            "mean": round(sum(values) / count, 1),
            "median": values[count // 2],
            "p95": values[min(count - 1, int(count * 0.95))],
        }

    categories = sorted({result.category for result in results})
    by_category = {}
    for category in categories:
        control_rows = [
            result for result in control if result.category == category and result.completed
        ]
        mcp_rows = [result for result in mcp if result.category == category and result.completed]
        control_tokens = sum(result.tokens_estimate for result in control_rows)
        mcp_tokens = sum(result.tokens_estimate for result in mcp_rows)
        by_category[category] = {
            "tasks": len(control_rows),
            "control_tokens": control_tokens,
            "mcp_tokens": mcp_tokens,
            "token_reduction_pct": round(
                (1 - mcp_tokens / max(control_tokens, 1)) * 100,
                1,
            ),
            "control_mean_ms": round(
                sum(result.wall_clock_ms for result in control_rows)
                / max(len(control_rows), 1),
                1,
            ),
            "mcp_mean_ms": round(
                sum(result.wall_clock_ms for result in mcp_rows) / max(len(mcp_rows), 1),
                1,
            ),
        }

    control_total_tokens = sum(result.tokens_estimate for result in control if result.completed)
    mcp_total_tokens = sum(result.tokens_estimate for result in mcp if result.completed)

    comparable_results = [result for result in results if result.task_id in comparable_task_ids]
    comparable_control = [result for result in comparable_results if result.arm == "control"]
    comparable_mcp = [result for result in comparable_results if result.arm == "mcp"]
    comparable_control_total_tokens = sum(
        result.tokens_estimate for result in comparable_control if result.completed
    )
    comparable_mcp_total_tokens = sum(
        result.tokens_estimate for result in comparable_mcp if result.completed
    )

    return {
        "total_tasks": len(TASKS),
        "comparable_tasks": COMPARABLE_TASK_COUNT,
        "mcp_native_tasks": MCP_NATIVE_TASK_COUNT,
        "control_completed": sum(1 for result in control if result.completed),
        "mcp_completed": sum(1 for result in mcp if result.completed),
        "tokens": {
            "control": stats(control, "tokens_estimate"),
            "mcp": stats(mcp, "tokens_estimate"),
            "total_reduction_pct": round(
                (1 - mcp_total_tokens / max(control_total_tokens, 1)) * 100,
                1,
            ),
        },
        "latency_ms": {
            "control": stats(control, "wall_clock_ms"),
            "mcp": stats(mcp, "wall_clock_ms"),
        },
        "tool_calls": {
            "control": stats(control, "tool_calls"),
            "mcp": stats(mcp, "tool_calls"),
        },
        "files_read": {
            "control": stats(control, "files_read"),
            "mcp": stats(mcp, "files_read"),
        },
        "comparable_subset": {
            "tasks": COMPARABLE_TASK_COUNT,
            "control_completed": sum(1 for result in comparable_control if result.completed),
            "mcp_completed": sum(1 for result in comparable_mcp if result.completed),
            "tokens": {
                "control": stats(comparable_control, "tokens_estimate"),
                "mcp": stats(comparable_mcp, "tokens_estimate"),
                "total_reduction_pct": round(
                    (1 - comparable_mcp_total_tokens / max(comparable_control_total_tokens, 1))
                    * 100,
                    1,
                ),
            },
            "latency_ms": {
                "control": stats(comparable_control, "wall_clock_ms"),
                "mcp": stats(comparable_mcp, "wall_clock_ms"),
            },
            "tool_calls": {
                "control": stats(comparable_control, "tool_calls"),
                "mcp": stats(comparable_mcp, "tool_calls"),
            },
            "files_read": {
                "control": stats(comparable_control, "files_read"),
                "mcp": stats(comparable_mcp, "files_read"),
            },
        },
        "by_category": by_category,
    }


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run the archived Contextro paired-study task suite.",
    )
    parser.add_argument(
        "--codebase",
        type=Path,
        default=Path(DEFAULT_CODEBASE) if DEFAULT_CODEBASE else None,
        help="Path to the original production monorepo checkout or an adapted equivalent repo.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=DEFAULT_OUTPUT_DIR,
        help=(
            "Directory where results.json, summary.json, config.json, and task catalogs "
            "will be written."
        ),
    )
    parser.add_argument(
        "--storage-dir",
        type=Path,
        default=None,
        help="Optional explicit CTX_STORAGE_DIR for the treatment arm.",
    )
    parser.add_argument(
        "--embedding-model",
        default="potion-code-16m",
        help="Embedding model for the treatment arm (default: potion-code-16m).",
    )
    parser.add_argument(
        "--export-task-catalogs-only",
        action="store_true",
        help=(
            "Write the machine-readable full and comparable task catalogs, then exit "
            "without touching the codebase or MCP runtime."
        ),
    )
    return parser.parse_args()


def _write_run_artifacts(
    output_dir: Path,
    results: list[TaskResult],
    summary: dict,
    codebase_path: Path,
    storage_dir: Path,
    embedding_model: str,
    task_catalog_paths: dict[str, Path],
) -> None:
    results_data = [asdict(result) for result in results]
    (output_dir / "results.json").write_text(json.dumps(results_data, indent=2) + "\n")
    (output_dir / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    (output_dir / "config.json").write_text(
        json.dumps(
            {
                "codebase": str(codebase_path),
                "tasks": len(TASKS),
                "comparable_tasks": COMPARABLE_TASK_COUNT,
                "mcp_native_tasks": MCP_NATIVE_TASK_COUNT,
                "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S"),
                "storage_dir": str(storage_dir),
                "embedding_model": embedding_model,
                "tokenizer": tokenizer_metadata(),
                "task_catalogs": {
                    "full": str(task_catalog_paths["full"]),
                    "comparable": str(task_catalog_paths["comparable"]),
                },
            },
            indent=2,
        )
        + "\n"
    )


async def main() -> None:
    args = _parse_args()
    output_dir = args.output_dir.expanduser().resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    task_catalog_paths = write_task_catalogs(output_dir)

    if args.export_task_catalogs_only:
        print("=" * 60)
        print("  EXPORTED PAIRED-STUDY TASK CATALOGS")
        print("=" * 60)
        print(f"  Total tasks:      {len(TASKS)}")
        print(f"  Comparable tasks: {COMPARABLE_TASK_COUNT}")
        print(f"  Full catalog:     {task_catalog_paths['full']}")
        print(f"  Comparable:       {task_catalog_paths['comparable']}")
        print("=" * 60)
        return

    if args.codebase is None:
        raise ValueError("--codebase is required unless --export-task-catalogs-only is used")

    codebase_path = args.codebase.expanduser().resolve()
    if not codebase_path.is_dir():
        raise FileNotFoundError(
            f"Codebase path does not exist or is not a directory: {codebase_path}"
        )

    storage_dir = (
        args.storage_dir.expanduser().resolve()
        if args.storage_dir is not None
        else (output_dir / ".contextro_experiment").resolve()
    )

    print("=" * 60)
    print("  CONTEXTRO MCP vs NO-MCP CONTROLLED EXPERIMENT")
    print("=" * 60)
    print(f"\n  Codebase:          {codebase_path}")
    print(f"  Tasks:             {len(TASKS)}")
    print(f"  Comparable tasks:  {COMPARABLE_TASK_COUNT}")
    print(f"  Output:            {output_dir}")
    print(
        "  Task catalogs:     "
        f"{FULL_TASK_CATALOG_FILENAME}, {COMPARABLE_TASK_CATALOG_FILENAME}\n"
    )

    _configure_environment(str(storage_dir), args.embedding_model)

    print("─── Initializing Contextro MCP ───")
    mcp, server_module = _reset_runtime()

    print(f"  Indexing {codebase_path}...")
    index_start = time.perf_counter()
    index_result = await index_and_wait(mcp, server_module, str(codebase_path))
    index_time = time.perf_counter() - index_start
    print(f"  Index complete: {index_result.get('total_files', '?')} files in {index_time:.1f}s")
    print("─── MCP Ready ───\n")

    results: list[TaskResult] = []

    print("─── Running Experiments ───\n")
    print(f"{'Task':<12} {'Category':<20} {'Ctrl Tokens':>12} {'MCP Tokens':>11} {'Savings':>8}")
    print("─" * 65)

    for task in TASKS:
        control_result = run_control_arm(task, str(codebase_path))
        results.append(control_result)

        mcp_result = await run_mcp_arm(task, mcp)
        results.append(mcp_result)

        if control_result.completed and mcp_result.completed:
            savings = round(
                (1 - mcp_result.tokens_estimate / max(control_result.tokens_estimate, 1))
                * 100
            )
            print(
                f"{task.id:<12} {task.category:<20} "
                f"{control_result.tokens_estimate:>12,} {mcp_result.tokens_estimate:>11,} "
                f"{savings:>7}%"
            )
            continue

        error = control_result.error or mcp_result.error
        print(f"{task.id:<12} {task.category:<20} {'ERROR':>12} {'':>11} {error[:20]}")

    print("\n─── Analysis ───\n")
    summary = analyze_results(results)

    print(
        f"  Tasks completed: {summary['control_completed']}/{summary['total_tasks']} (control), "
        f"{summary['mcp_completed']}/{summary['total_tasks']} (MCP)"
    )
    print()
    print(f"  {'Metric':<20} {'Control':>12} {'MCP':>12} {'Reduction':>10}")
    print(f"  {'─' * 54}")
    print(
        f"  {'Total tokens':<20} {summary['tokens']['control']['sum']:>12,} "
        f"{summary['tokens']['mcp']['sum']:>12,} "
        f"{summary['tokens']['total_reduction_pct']:>9.1f}%"
    )
    print(
        f"  {'Mean tokens/task':<20} {summary['tokens']['control']['mean']:>12,.0f} "
        f"{summary['tokens']['mcp']['mean']:>12,.0f}"
    )
    print(
        f"  {'Median latency ms':<20} {summary['latency_ms']['control']['median']:>12,.1f} "
        f"{summary['latency_ms']['mcp']['median']:>12,.1f}"
    )
    print(
        f"  {'Mean tool calls':<20} {summary['tool_calls']['control']['mean']:>12,.1f} "
        f"{summary['tool_calls']['mcp']['mean']:>12,.1f}"
    )
    print(
        f"  {'Mean files read':<20} {summary['files_read']['control']['mean']:>12,.1f} "
        f"{summary['files_read']['mcp']['mean']:>12,.1f}"
    )

    print("\n  By Category:")
    print(f"  {'Category':<22} {'Ctrl Tok':>9} {'MCP Tok':>9} {'Reduction':>10}")
    print(f"  {'─' * 52}")
    for category, data in summary["by_category"].items():
        print(
            f"  {category:<22} {data['control_tokens']:>9,} "
            f"{data['mcp_tokens']:>9,} {data['token_reduction_pct']:>9.1f}%"
        )

    _write_run_artifacts(
        output_dir=output_dir,
        results=results,
        summary=summary,
        codebase_path=codebase_path,
        storage_dir=storage_dir,
        embedding_model=args.embedding_model,
        task_catalog_paths=task_catalog_paths,
    )

    print(f"\n  Results saved to: {output_dir}/")
    print("=" * 60)


if __name__ == "__main__":
    asyncio.run(main())
