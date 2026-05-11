"""Run a public proxy benchmark with a stronger local-tools baseline."""

# ruff: noqa: E402, I001

from __future__ import annotations

import argparse
import asyncio
import gc
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from dataclasses import asdict, dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "src"))

from benchmark_utils import call_tool, estimate_tokens, index_codebase

from contextro_mcp.token_counting import tokenizer_metadata
from public_proxy_repo import (
    DEFAULT_PROXY_REPO,
    DISCOVERY_TASKS,
    ProxyDiscoveryTask,
    export_task_catalogs,
    materialize_public_proxy_repo,
    seed_git_history,
)


@dataclass
class BenchmarkResult:
    task_id: str
    arm: str
    category: str
    completed: bool
    success: bool
    wall_clock_ms: float
    tokens_estimate: int
    tool_calls: int
    files_read: int
    evidence: list[str]
    error: str = ""


def _stats(results: list[BenchmarkResult], attr: str) -> dict[str, float]:
    values = [getattr(result, attr) for result in results if result.completed]
    if not values:
        return {"count": 0, "sum": 0.0, "mean": 0.0, "median": 0.0, "p95": 0.0}
    ordered = sorted(values)
    count = len(ordered)
    p95_index = min(count - 1, max(0, int(count * 0.95) - 1))
    return {
        "count": count,
        "sum": round(sum(ordered), 2),
        "mean": round(sum(ordered) / count, 2),
        "median": round(statistics.median(ordered), 2),
        "p95": round(ordered[p95_index], 2),
    }


def _parse_candidate_files(output: str) -> list[str]:
    files: list[str] = []
    for line in output.splitlines():
        if not line.strip():
            continue
        if ":" in line:
            candidate = line.split(":", 1)[0].strip()
            if candidate and candidate not in files:
                files.append(candidate)
    return files


def _run_command(repo_root: Path, *args: str) -> str:
    completed = subprocess.run(
        args,
        cwd=repo_root,
        check=False,
        capture_output=True,
        text=True,
        env={**os.environ, "LC_ALL": "C"},
    )
    return (completed.stdout + completed.stderr).strip()


def run_stronger_local_arm(task: ProxyDiscoveryTask, repo_root: Path) -> BenchmarkResult:
    started = time.perf_counter()
    combined_text: list[str] = []
    candidate_files: list[str] = []
    files_read = 0
    tool_calls = 0

    try:
        for step in task.baseline_steps:
            tool_calls += 1
            if step.tool == "git_grep":
                output = _run_command(
                    repo_root,
                    "git",
                    "grep",
                    "-n",
                    "-I",
                    step.query,
                    "--",
                    step.path,
                )
            elif step.tool == "rg":
                output = _run_command(
                    repo_root,
                    "git",
                    "grep",
                    "-n",
                    "-I",
                    "-E",
                    step.query,
                    step.path,
                )
            elif step.tool == "git_log":
                output = _run_command(
                    repo_root,
                    "git",
                    "log",
                    "--oneline",
                    "--grep",
                    step.query,
                    "--",
                    step.path,
                )
            else:
                raise ValueError(f"Unsupported baseline step: {step.tool}")

            combined_text.append(output)
            if step.tool in {"git_grep", "rg"}:
                for candidate in _parse_candidate_files(output)[: step.max_results]:
                    if candidate not in candidate_files:
                        candidate_files.append(candidate)

        for relative_path in candidate_files[: task.max_reads]:
            tool_calls += 1
            files_read += 1
            file_text = (repo_root / relative_path).read_text(encoding="utf-8", errors="replace")
            combined_text.append(file_text)

        blob = "\n".join(combined_text)
        success = any(expected in blob for expected in task.expected_any)
        return BenchmarkResult(
            task_id=task.id,
            arm="stronger_local",
            category=task.category,
            completed=True,
            success=success,
            wall_clock_ms=round((time.perf_counter() - started) * 1000, 2),
            tokens_estimate=estimate_tokens({"baseline": blob}),
            tool_calls=tool_calls,
            files_read=files_read,
            evidence=task.expected_any[:3],
        )
    except Exception as exc:  # pragma: no cover - surfaced in artifact
        return BenchmarkResult(
            task_id=task.id,
            arm="stronger_local",
            category=task.category,
            completed=False,
            success=False,
            wall_clock_ms=round((time.perf_counter() - started) * 1000, 2),
            tokens_estimate=0,
            tool_calls=tool_calls,
            files_read=files_read,
            evidence=[],
            error=str(exc),
        )


async def run_contextro_arm(task: ProxyDiscoveryTask, mcp) -> BenchmarkResult:
    started = time.perf_counter()
    result = await call_tool(mcp, task.mcp_tool, task.mcp_args)
    blob = json.dumps(result, sort_keys=True)
    has_error = bool(result.get("error"))
    return BenchmarkResult(
        task_id=task.id,
        arm="contextro",
        category=task.category,
        completed=not has_error,
        success=not has_error and any(expected in blob for expected in task.expected_any),
        wall_clock_ms=round((time.perf_counter() - started) * 1000, 2),
        tokens_estimate=estimate_tokens(result),
        tool_calls=1,
        files_read=0,
        evidence=list(task.expected_any[:3]),
        error=str(result.get("error", "")) if has_error else "",
    )


def _configure_environment(storage_dir: Path) -> None:
    os.environ["CTX_STORAGE_DIR"] = str(storage_dir)
    os.environ["CTX_PERMISSION_LEVEL"] = "full"
    os.environ.setdefault("CTX_EMBEDDING_MODEL", "potion-code-16m")


def _reset_runtime():
    import contextro_mcp.server as server_module
    from contextro_mcp.config import reset_settings
    from contextro_mcp.state import reset_state

    reset_settings()
    reset_state()
    server_module._pipeline = None
    server_module._index_job = {}
    return server_module.create_server(), server_module


def _shutdown_runtime() -> None:
    from contextro_mcp.config import reset_settings
    from contextro_mcp.state import get_state, reset_state
    import contextro_mcp.server as server_module

    try:
        get_state().shutdown()
    except Exception:
        pass
    reset_settings()
    reset_state()
    server_module._pipeline = None
    server_module._index_job = {}
    gc.collect()


def _summarize(results: list[BenchmarkResult]) -> dict[str, object]:
    stronger = [result for result in results if result.arm == "stronger_local"]
    contextro = [result for result in results if result.arm == "contextro"]
    by_category: dict[str, dict[str, object]] = {}
    for category in sorted({result.category for result in results}):
        strong_rows = [result for result in stronger if result.category == category]
        ctx_rows = [result for result in contextro if result.category == category]
        strong_tokens = sum(result.tokens_estimate for result in strong_rows if result.completed)
        ctx_tokens = sum(result.tokens_estimate for result in ctx_rows if result.completed)
        by_category[category] = {
            "tasks": len(strong_rows),
            "stronger_local_success_rate": round(
                sum(1 for result in strong_rows if result.success) / max(len(strong_rows), 1), 3
            ),
            "contextro_success_rate": round(
                sum(1 for result in ctx_rows if result.success) / max(len(ctx_rows), 1), 3
            ),
            "stronger_local_tokens": strong_tokens,
            "contextro_tokens": ctx_tokens,
            "token_reduction_pct": round((1 - ctx_tokens / max(strong_tokens, 1)) * 100, 1),
        }

    stronger_tokens = sum(result.tokens_estimate for result in stronger if result.completed)
    contextro_tokens = sum(result.tokens_estimate for result in contextro if result.completed)

    return {
        "tasks": len(DISCOVERY_TASKS),
        "tokenizer": tokenizer_metadata(),
        "arms": {
            "stronger_local": {
                "completed": sum(1 for result in stronger if result.completed),
                "successful": sum(1 for result in stronger if result.success),
                "success_rate": round(
                    sum(1 for result in stronger if result.success) / max(len(stronger), 1), 3
                ),
                "tokens": _stats(stronger, "tokens_estimate"),
                "latency_ms": _stats(stronger, "wall_clock_ms"),
                "tool_calls": _stats(stronger, "tool_calls"),
                "files_read": _stats(stronger, "files_read"),
            },
            "contextro": {
                "completed": sum(1 for result in contextro if result.completed),
                "successful": sum(1 for result in contextro if result.success),
                "success_rate": round(
                    sum(1 for result in contextro if result.success) / max(len(contextro), 1), 3
                ),
                "tokens": _stats(contextro, "tokens_estimate"),
                "latency_ms": _stats(contextro, "wall_clock_ms"),
                "tool_calls": _stats(contextro, "tool_calls"),
                "files_read": _stats(contextro, "files_read"),
            },
        },
        "overall_token_reduction_pct": round(
            (1 - contextro_tokens / max(stronger_tokens, 1)) * 100,
            1,
        ),
        "by_category": by_category,
        "results": [asdict(result) for result in results],
    }


async def run_benchmark(
    proxy_repo_root: Path,
    output_path: Path | None = None,
    *,
    regenerate_proxy_repo: bool = False,
) -> dict[str, object]:
    if regenerate_proxy_repo or not proxy_repo_root.exists():
        materialize_public_proxy_repo(proxy_repo_root)

    export_task_catalogs(ROOT / "docs" / "publication")
    temp_dir = Path(tempfile.mkdtemp(prefix="ctx_public_proxy_"))
    working_repo = temp_dir / "repo"
    storage_dir = temp_dir / ".contextro"
    shutil.copytree(proxy_repo_root, working_repo)
    commit_messages = seed_git_history(working_repo)
    storage_dir.mkdir()

    _configure_environment(storage_dir)
    mcp, server_module = _reset_runtime()
    try:
        index_result = await index_codebase(
            mcp,
            server_module,
            str(working_repo),
            timeout_seconds=300,
        )
        results: list[BenchmarkResult] = []
        for task in DISCOVERY_TASKS:
            results.append(run_stronger_local_arm(task, working_repo))
            results.append(await run_contextro_arm(task, mcp))

        summary = _summarize(results)
        summary["proxy_repo"] = {
            "tracked_files": sum(1 for path in proxy_repo_root.rglob("*") if path.is_file()),
            "working_files": sum(1 for path in working_repo.rglob("*") if path.is_file()),
            "python_files": sum(1 for path in working_repo.rglob("*.py")),
            "commit_messages": commit_messages,
            "index": index_result,
        }
        if output_path is not None:
            output_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
        return summary
    finally:
        _shutdown_runtime()
        shutil.rmtree(temp_dir, ignore_errors=True)


def main() -> dict[str, object]:
    parser = argparse.ArgumentParser(
        description="Run the public proxy stronger-baseline benchmark."
    )
    parser.add_argument(
        "--proxy-repo",
        type=Path,
        default=DEFAULT_PROXY_REPO,
        help="Path to the tracked public proxy repository.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "docs" / "publication" / "public-proxy-strong-baseline.json",
        help="Where to write the benchmark JSON summary.",
    )
    parser.add_argument(
        "--regenerate-proxy-repo",
        action="store_true",
        help="Regenerate the tracked proxy repo before benchmarking.",
    )
    args = parser.parse_args()

    metrics = asyncio.run(
        run_benchmark(
            args.proxy_repo.resolve(),
            args.output.resolve(),
            regenerate_proxy_repo=args.regenerate_proxy_repo,
        )
    )
    print(json.dumps(metrics["arms"], indent=2))
    print(f"Results saved to: {args.output}")
    return metrics


if __name__ == "__main__":
    main()
