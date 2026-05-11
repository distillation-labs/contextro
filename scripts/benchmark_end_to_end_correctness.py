"""Deterministic end-to-end edit correctness benchmark for publication artifacts."""

# ruff: noqa: E402, I001

from __future__ import annotations

import argparse
import asyncio
import gc
import json
import os
import py_compile
import shutil
import statistics
import sys
import tempfile
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from textwrap import dedent

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "src"))

from benchmark_utils import call_tool, estimate_tokens, index_codebase
from contextro_mcp.token_counting import tokenizer_metadata


@dataclass(frozen=True)
class EditTask:
    id: str
    prompt: str
    discovery_tool: str
    discovery_args: dict[str, object]
    baseline_pattern: str
    target_file: str
    pattern: str
    replacement: str
    expected_contains: tuple[str, ...]
    expected_not_contains: tuple[str, ...] = ()


@dataclass
class EditResult:
    task_id: str
    arm: str
    completed: bool
    success: bool
    wall_clock_ms: float
    tokens_estimate: int
    tool_calls: int
    files_read: int
    target_file: str
    error: str = ""


TASKS: tuple[EditTask, ...] = (
    EditTask(
        id="edit_correctness_01",
        prompt="Increase the session grace window constant used by stale-session revocation.",
        discovery_tool="find_symbol",
        discovery_args={"name": "revoke_stale_session_token"},
        baseline_pattern="SESSION_GRACE_MINUTES",
        target_file="packages/auth/src/auth/session_tokens.py",
        pattern="SESSION_GRACE_MINUTES = 20",
        replacement="SESSION_GRACE_MINUTES = 30",
        expected_contains=("SESSION_GRACE_MINUTES = 30",),
        expected_not_contains=("SESSION_GRACE_MINUTES = 20",),
    ),
    EditTask(
        id="edit_correctness_02",
        prompt="Make email and slack the default notification channels.",
        discovery_tool="find_symbol",
        discovery_args={"name": "resolve_notification_channels"},
        baseline_pattern="DEFAULT_CHANNELS",
        target_file="packages/notifications/src/notifications/preferences.py",
        pattern='DEFAULT_CHANNELS = ("email",)',
        replacement='DEFAULT_CHANNELS = ("email", "slack")',
        expected_contains=('DEFAULT_CHANNELS = ("email", "slack")',),
        expected_not_contains=('DEFAULT_CHANNELS = ("email",)',),
    ),
    EditTask(
        id="edit_correctness_03",
        prompt="Lower the rollout minimum sample size threshold.",
        discovery_tool="find_symbol",
        discovery_args={"name": "evaluate_rollout_gate"},
        baseline_pattern="MIN_SAMPLE_SIZE",
        target_file="packages/experiments/src/experiments/rollouts.py",
        pattern="MIN_SAMPLE_SIZE = 200",
        replacement="MIN_SAMPLE_SIZE = 150",
        expected_contains=("MIN_SAMPLE_SIZE = 150",),
        expected_not_contains=("MIN_SAMPLE_SIZE = 200",),
    ),
    EditTask(
        id="edit_correctness_04",
        prompt="Change the partner metric prefix to the activity namespace.",
        discovery_tool="find_symbol",
        discovery_args={"name": "emit_partner_metric"},
        baseline_pattern="PARTNER_METRIC_PREFIX",
        target_file="packages/analytics/src/analytics/reporter.py",
        pattern='PARTNER_METRIC_PREFIX = "partners.lifecycle"',
        replacement='PARTNER_METRIC_PREFIX = "partners.activity"',
        expected_contains=('PARTNER_METRIC_PREFIX = "partners.activity"',),
        expected_not_contains=('PARTNER_METRIC_PREFIX = "partners.lifecycle"',),
    ),
    EditTask(
        id="edit_correctness_05",
        prompt="Increase the projection batch size used by search refreshes.",
        discovery_tool="find_symbol",
        discovery_args={"name": "refresh_partner_search_projection"},
        baseline_pattern="PROJECTION_BATCH_SIZE",
        target_file="packages/search/src/search/projections.py",
        pattern="PROJECTION_BATCH_SIZE = 100",
        replacement="PROJECTION_BATCH_SIZE = 120",
        expected_contains=("PROJECTION_BATCH_SIZE = 120",),
        expected_not_contains=("PROJECTION_BATCH_SIZE = 100",),
    ),
    EditTask(
        id="edit_correctness_06",
        prompt="Reduce the default retry backoff factor for digest delivery jobs.",
        discovery_tool="find_symbol",
        discovery_args={"name": "build_retry_plan"},
        baseline_pattern="DEFAULT_BACKOFF_FACTOR",
        target_file="packages/workflows/src/workflows/retry_policies.py",
        pattern="DEFAULT_BACKOFF_FACTOR = 2.0",
        replacement="DEFAULT_BACKOFF_FACTOR = 1.5",
        expected_contains=("DEFAULT_BACKOFF_FACTOR = 1.5",),
        expected_not_contains=("DEFAULT_BACKOFF_FACTOR = 2.0",),
    ),
    EditTask(
        id="edit_correctness_07",
        prompt="Adjust the default digest delay for batched partner notifications.",
        discovery_tool="find_symbol",
        discovery_args={"name": "schedule_digest_delivery"},
        baseline_pattern="DEFAULT_DIGEST_DELAY_MINUTES",
        target_file="packages/notifications/src/notifications/digest_scheduler.py",
        pattern="DEFAULT_DIGEST_DELAY_MINUTES = 45",
        replacement="DEFAULT_DIGEST_DELAY_MINUTES = 60",
        expected_contains=("DEFAULT_DIGEST_DELAY_MINUTES = 60",),
        expected_not_contains=("DEFAULT_DIGEST_DELAY_MINUTES = 45",),
    ),
    EditTask(
        id="edit_correctness_08",
        prompt="Rename the onboarding review audit event to screened.",
        discovery_tool="search",
        discovery_args={"query": "partner onboarding review audit event", "limit": 5},
        baseline_pattern="partner_onboarding.reviewed",
        target_file="packages/partners/src/partners/onboarding.py",
        pattern='"partner_onboarding.reviewed"',
        replacement='"partner_onboarding.screened"',
        expected_contains=('"partner_onboarding.screened"',),
        expected_not_contains=('"partner_onboarding.reviewed"',),
    ),
)


def create_edit_codebase(root: Path) -> Path:
    files = {
        "packages/auth/src/auth/session_tokens.py": dedent(
            '''
            from __future__ import annotations

            SESSION_GRACE_MINUTES = 20


            def revoke_stale_session_token(actor_id: str) -> str:
                return f"revoke:{actor_id}:{SESSION_GRACE_MINUTES}"
            '''
        ),
        "packages/notifications/src/notifications/preferences.py": dedent(
            '''
            from __future__ import annotations

            DEFAULT_CHANNELS = ("email",)


            def resolve_notification_channels(blob: dict[str, object]) -> tuple[str, ...]:
                return tuple(blob.get("channels", DEFAULT_CHANNELS))
            '''
        ),
        "packages/experiments/src/experiments/rollouts.py": dedent(
            '''
            from __future__ import annotations

            MIN_SAMPLE_SIZE = 200


            def evaluate_rollout_gate(actor_id: str, sample_size: int, bucket: int) -> bool:
                return sample_size >= MIN_SAMPLE_SIZE and bucket % 10 < 3 and actor_id != "blocked"
            '''
        ),
        "packages/analytics/src/analytics/reporter.py": dedent(
            '''
            from __future__ import annotations

            PARTNER_METRIC_PREFIX = "partners.lifecycle"


            def emit_partner_metric(metric_name: str, actor_id: str) -> str:
                return f"{PARTNER_METRIC_PREFIX}.{metric_name}:{actor_id}"
            '''
        ),
        "packages/search/src/search/projections.py": dedent(
            '''
            from __future__ import annotations

            PROJECTION_BATCH_SIZE = 100


            def refresh_partner_search_projection(
                partner_id: str,
                profile: dict[str, object],
            ) -> dict[str, object]:
                return {
                    "partner_id": partner_id,
                    "profile": profile,
                    "batch_size": PROJECTION_BATCH_SIZE,
                }
            '''
        ),
        "packages/workflows/src/workflows/retry_policies.py": dedent(
            '''
            from __future__ import annotations

            DEFAULT_BACKOFF_FACTOR = 2.0


            def build_retry_plan(job_name: str) -> dict[str, object]:
                return {
                    "job_name": job_name,
                    "attempts": 4,
                    "backoff_factor": DEFAULT_BACKOFF_FACTOR,
                }
            '''
        ),
        "packages/notifications/src/notifications/digest_scheduler.py": dedent(
            '''
            from __future__ import annotations

            DEFAULT_DIGEST_DELAY_MINUTES = 45


            def schedule_digest_delivery(
                account_name: str,
                delay_minutes: int = DEFAULT_DIGEST_DELAY_MINUTES,
            ) -> dict[str, object]:
                return {"account_name": account_name, "delay_minutes": delay_minutes}
            '''
        ),
        "packages/partners/src/partners/onboarding.py": dedent(
            '''
            from __future__ import annotations


            def prepare_partner_onboarding_context(alias: str, actor_id: str) -> dict[str, object]:
                event_name = "partner_onboarding.reviewed"
                return {"alias": alias, "actor_id": actor_id, "event_name": event_name}
            '''
        ),
        "apps/console/src/console/dashboard.py": dedent(
            '''
            from __future__ import annotations

            from analytics.reporter import emit_partner_metric
            from experiments.rollouts import evaluate_rollout_gate
            from partners.onboarding import prepare_partner_onboarding_context


            def build_partner_dashboard(alias: str, actor_id: str) -> dict[str, object]:
                return {
                    "context": prepare_partner_onboarding_context(alias, actor_id),
                    "metric": emit_partner_metric("dashboard.rendered", actor_id),
                    "rollout": evaluate_rollout_gate(actor_id, 300, 2),
                }
            '''
        ),
        "apps/worker/src/worker/digests.py": dedent(
            '''
            from __future__ import annotations

            from notifications.digest_scheduler import schedule_digest_delivery
            from notifications.preferences import resolve_notification_channels


            def run_digest(account_name: str, blob: dict[str, object]) -> dict[str, object]:
                schedule = schedule_digest_delivery(account_name)
                schedule["channels"] = resolve_notification_channels(blob)
                return schedule
            '''
        ),
        "apps/worker/src/worker/search_refresh.py": dedent(
            '''
            from __future__ import annotations

            from search.projections import refresh_partner_search_projection


            def run_search_refresh(partner_id: str) -> dict[str, object]:
                return refresh_partner_search_projection(partner_id, {"status": "active"})
            '''
        ),
        "apps/worker/src/worker/audit.py": dedent(
            '''
            from __future__ import annotations

            from analytics.reporter import emit_partner_metric
            from partners.onboarding import prepare_partner_onboarding_context


            def replay_audit(alias: str, actor_id: str) -> dict[str, object]:
                metric = emit_partner_metric("audit.replayed", actor_id)
                context = prepare_partner_onboarding_context(alias, actor_id)
                return {"metric": metric, "context": context}
            '''
        ),
    }

    for relative_path, content in files.items():
        path = root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content.strip() + "\n", encoding="utf-8")
    return root


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


def _parse_candidate_files(output: str) -> list[str]:
    files: list[str] = []
    for line in output.splitlines():
        if ":" not in line:
            continue
        candidate = line.split(":", 1)[0].strip()
        if candidate and candidate not in files:
            files.append(candidate)
    return files


def _assert_task_success(task: EditTask, repo_root: Path) -> bool:
    text = (repo_root / task.target_file).read_text(encoding="utf-8")
    return all(needle in text for needle in task.expected_contains) and all(
        needle not in text for needle in task.expected_not_contains
    )


def _syntax_check(path: Path) -> bool:
    try:
        py_compile.compile(str(path), doraise=True)
        return True
    except py_compile.PyCompileError:
        return False


def run_baseline_arm(task: EditTask, repo_root: Path) -> EditResult:
    import subprocess

    started = time.perf_counter()
    command = [
        "grep",
        "-R",
        "-n",
        "--include=*.py",
        task.baseline_pattern,
        str(repo_root),
    ]
    completed = subprocess.run(command, capture_output=True, text=True, check=False)
    output = completed.stdout + completed.stderr
    candidates = _parse_candidate_files(output)
    files_read = 0
    target_path = None
    for candidate in candidates[:3]:
        candidate_path = Path(candidate)
        if task.pattern in candidate_path.read_text(encoding="utf-8", errors="replace"):
            files_read += 1
            target_path = candidate_path
            break
        files_read += 1
    if target_path is None:
        return EditResult(
            task_id=task.id,
            arm="stronger_local",
            completed=False,
            success=False,
            wall_clock_ms=round((time.perf_counter() - started) * 1000, 2),
            tokens_estimate=estimate_tokens({"baseline": output}),
            tool_calls=1 + files_read,
            files_read=files_read,
            target_file=task.target_file,
            error="pattern not found",
        )

    text = target_path.read_text(encoding="utf-8")
    target_path.write_text(text.replace(task.pattern, task.replacement), encoding="utf-8")
    success = _assert_task_success(task, repo_root) and _syntax_check(target_path)
    return EditResult(
        task_id=task.id,
        arm="stronger_local",
        completed=True,
        success=success,
        wall_clock_ms=round((time.perf_counter() - started) * 1000, 2),
        tokens_estimate=estimate_tokens({"baseline": output, "file": text}),
        tool_calls=1 + files_read,
        files_read=files_read,
        target_file=task.target_file,
    )


def _extract_target_file(result: dict, fallback: str) -> str:
    if "file" in result and result["file"]:
        return str(result["file"])
    for item in result.get("results", []):
        for key in ("file", "f", "filepath"):
            if key in item and item[key]:
                return str(item[key])
    for item in result.get("symbols", []):
        if "f" in item and item["f"]:
            return str(item["f"])
        location = item.get("location")
        if isinstance(location, dict) and location.get("file"):
            return str(location["file"])
    return fallback


async def run_contextro_arm(task: EditTask, repo_root: Path) -> EditResult:
    temp_storage = repo_root / ".contextro"
    temp_storage.mkdir(exist_ok=True)
    _configure_environment(temp_storage)
    mcp, server_module = _reset_runtime()
    try:
        await index_codebase(mcp, server_module, str(repo_root), timeout_seconds=180)
        started = time.perf_counter()
        discovery = await call_tool(mcp, task.discovery_tool, task.discovery_args)
        target_file = _extract_target_file(discovery, task.target_file)
        preview = await call_tool(
            mcp,
            "code",
            {
                "operation": "pattern_rewrite",
                "file_path": target_file,
                "pattern": task.pattern,
                "replacement": task.replacement,
                "language": "python",
                "dry_run": True,
            },
        )
        apply = await call_tool(
            mcp,
            "code",
            {
                "operation": "pattern_rewrite",
                "file_path": target_file,
                "pattern": task.pattern,
                "replacement": task.replacement,
                "language": "python",
                "dry_run": False,
            },
        )
        success = (
            not discovery.get("error")
            and not preview.get("error")
            and not apply.get("error")
            and _assert_task_success(task, repo_root)
            and _syntax_check(repo_root / task.target_file)
        )
        return EditResult(
            task_id=task.id,
            arm="contextro",
            completed=True,
            success=success,
            wall_clock_ms=round((time.perf_counter() - started) * 1000, 2),
            tokens_estimate=estimate_tokens(
                {"discovery": discovery, "preview": preview, "apply": apply}
            ),
            tool_calls=3,
            files_read=0,
            target_file=target_file,
        )
    except Exception as exc:  # pragma: no cover - surfaced in artifact
        return EditResult(
            task_id=task.id,
            arm="contextro",
            completed=False,
            success=False,
            wall_clock_ms=0.0,
            tokens_estimate=0,
            tool_calls=0,
            files_read=0,
            target_file=task.target_file,
            error=str(exc),
        )
    finally:
        _shutdown_runtime()


def _stats(results: list[EditResult], attr: str) -> dict[str, float]:
    values = [getattr(result, attr) for result in results if result.completed]
    if not values:
        return {"count": 0, "sum": 0.0, "mean": 0.0, "median": 0.0}
    return {
        "count": len(values),
        "sum": round(sum(values), 2),
        "mean": round(sum(values) / len(values), 2),
        "median": round(statistics.median(values), 2),
    }


async def run_benchmark(output_path: Path | None = None) -> dict[str, object]:
    results: list[EditResult] = []
    for task in TASKS:
        baseline_dir = Path(tempfile.mkdtemp(prefix=f"ctx_edit_baseline_{task.id}_"))
        contextro_dir = Path(tempfile.mkdtemp(prefix=f"ctx_edit_contextro_{task.id}_"))
        try:
            create_edit_codebase(baseline_dir)
            create_edit_codebase(contextro_dir)
            results.append(run_baseline_arm(task, baseline_dir))
            results.append(await run_contextro_arm(task, contextro_dir))
        finally:
            shutil.rmtree(baseline_dir, ignore_errors=True)
            shutil.rmtree(contextro_dir, ignore_errors=True)

    stronger = [result for result in results if result.arm == "stronger_local"]
    contextro = [result for result in results if result.arm == "contextro"]
    summary = {
        "tasks": len(TASKS),
        "tokenizer": tokenizer_metadata(),
        "arms": {
            "stronger_local": {
                "successful": sum(1 for result in stronger if result.success),
                "success_rate": round(
                    sum(1 for result in stronger if result.success) / max(len(stronger), 1),
                    3,
                ),
                "tokens": _stats(stronger, "tokens_estimate"),
                "latency_ms": _stats(stronger, "wall_clock_ms"),
                "tool_calls": _stats(stronger, "tool_calls"),
            },
            "contextro": {
                "successful": sum(1 for result in contextro if result.success),
                "success_rate": round(
                    sum(1 for result in contextro if result.success) / max(len(contextro), 1),
                    3,
                ),
                "tokens": _stats(contextro, "tokens_estimate"),
                "latency_ms": _stats(contextro, "wall_clock_ms"),
                "tool_calls": _stats(contextro, "tool_calls"),
            },
        },
        "results": [asdict(result) for result in results],
    }
    if output_path is not None:
        output_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    return summary


def main() -> dict[str, object]:
    parser = argparse.ArgumentParser(description="Run the end-to-end edit correctness benchmark.")
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "docs" / "publication" / "edit-correctness-benchmark.json",
        help="Where to write the benchmark JSON summary.",
    )
    args = parser.parse_args()
    summary = asyncio.run(run_benchmark(args.output.resolve()))
    print(json.dumps(summary["arms"], indent=2))
    print(f"Results saved to: {args.output}")
    return summary


if __name__ == "__main__":
    main()
