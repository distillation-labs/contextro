"""Benchmark edit-assistance planning, preview, and apply flows."""

from __future__ import annotations

import asyncio
import json
import sys
import tempfile
import time
from dataclasses import asdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from benchmark_utils import benchmark_session, call_tool, estimate_tokens, index_codebase

from contextro_mcp.editing.benchmarking import (
    EditBenchmarkRun,
    EditBenchmarkTask,
    EditConstraints,
    EditExpected,
    EditOperation,
    TextAssertion,
    evaluate_text_assertions,
    score_run,
)


def create_edit_benchmark_codebase(tmp_dir: Path) -> Path:
    """Create a small deterministic codebase for edit-assistance evaluation."""
    src = tmp_dir / "src"
    tests = tmp_dir / "tests"
    src.mkdir(parents=True, exist_ok=True)
    tests.mkdir(parents=True, exist_ok=True)

    (src / "utils.py").write_text(
        "def helper():\n"
        '    print("old")\n\n'
        "def helper_alias():\n"
        "    return helper()\n"
    )
    (src / "main.py").write_text(
        "from utils import helper\n\n\n"
        "def run():\n"
        "    helper()\n"
    )
    (src / "secondary.py").write_text(
        "def alternate():\n"
        '    print("old")\n'
    )
    (tests / "test_main.py").write_text(
        "from src.main import run\n\n\n"
        "def test_run():\n"
        "    run()\n"
    )
    return tmp_dir


TASKS: tuple[EditBenchmarkTask, ...] = (
    EditBenchmarkTask(
        id="plan_01",
        title="Plan single-file print replacement",
        phase="phase1",
        mode="plan",
        category="single_file_call_replacement",
        language="python",
        scope="single_file",
        instruction="Plan a replacement of print('old') with print('new') in utils.py",
        operation=EditOperation(
            name="edit_plan",
            file_path="src/utils.py",
            pattern='print("old")',
            replacement='print("new")',
            language="python",
            goal="Replace the helper print call",
        ),
        constraints=EditConstraints(allow_apply=False, require_dry_run=False),
        expected=EditExpected(
            primary_target_file="src/utils.py",
            target_files=("src/utils.py",),
            impacted_files=(),
            verify_labels=("syntax", "lint", "unit_tests"),
            risk_labels=(),
        ),
        score_profile="plan_only",
    ),
    EditBenchmarkTask(
        id="preview_01",
        title="Preview single-file print replacement",
        phase="phase2",
        mode="preview",
        category="single_file_call_replacement",
        language="python",
        scope="single_file",
        instruction="Preview a replacement of print('old') with print('new') in utils.py",
        operation=EditOperation(
            name="pattern_rewrite",
            file_path="src/utils.py",
            pattern='print("old")',
            replacement='print("new")',
            language="python",
            dry_run=True,
        ),
        constraints=EditConstraints(allow_apply=False, require_dry_run=True),
        expected=EditExpected(
            primary_target_file="src/utils.py",
            preview_files=("src/utils.py",),
            expected_change_count=1,
            verify_labels=("syntax",),
            preview_assertions=(
                TextAssertion(source="preview", kind="contains", text="@@"),
                TextAssertion(source="preview", kind="contains", text='print("new")'),
            ),
        ),
        score_profile="preview",
    ),
    EditBenchmarkTask(
        id="plan_02",
        title="Plan directory-scoped replacement",
        phase="phase3",
        mode="plan",
        category="directory_scoped_rewrite",
        language="python",
        scope="multi_file",
        instruction="Plan a scoped replacement of print('old') with print('new') across src",
        operation=EditOperation(
            name="edit_plan",
            path="src",
            pattern='print("old")',
            replacement='print("new")',
            language="python",
            goal="Replace print calls across src",
        ),
        constraints=EditConstraints(allow_apply=False, require_dry_run=False),
        expected=EditExpected(
            primary_target_file="src/secondary.py",
            target_files=("src/secondary.py", "src/utils.py"),
            verify_labels=("syntax", "lint", "unit_tests"),
            risk_labels=("broad_rewrite",),
        ),
        score_profile="multi_file_plan",
    ),
    EditBenchmarkTask(
        id="preview_02",
        title="Preview directory-scoped replacement",
        phase="phase2",
        mode="preview",
        category="directory_scoped_rewrite",
        language="python",
        scope="multi_file",
        instruction="Preview a replacement of print('old') with print('new') across src",
        operation=EditOperation(
            name="pattern_rewrite",
            path="src",
            pattern='print("old")',
            replacement='print("new")',
            language="python",
            dry_run=True,
        ),
        constraints=EditConstraints(allow_apply=False, require_dry_run=True),
        expected=EditExpected(
            primary_target_file="src/secondary.py",
            preview_files=("src/secondary.py", "src/utils.py"),
            expected_change_count=2,
            verify_labels=("syntax",),
            preview_assertions=(
                TextAssertion(source="preview", kind="contains", text='print("new")'),
            ),
        ),
        score_profile="preview",
    ),
    EditBenchmarkTask(
        id="apply_01",
        title="Apply single-file print replacement",
        phase="phase2",
        mode="apply",
        category="single_file_call_replacement",
        language="python",
        scope="single_file",
        instruction="Apply a replacement of print('old') with print('new') in utils.py",
        operation=EditOperation(
            name="pattern_rewrite",
            file_path="src/utils.py",
            pattern='print("old")',
            replacement='print("new")',
            language="python",
            dry_run=False,
        ),
        constraints=EditConstraints(allow_apply=True, require_dry_run=True),
        expected=EditExpected(
            primary_target_file="src/utils.py",
            apply_files=("src/utils.py",),
            expected_change_count=1,
            verify_labels=("syntax",),
            post_apply_assertions=(
                TextAssertion(path="src/utils.py", kind="contains", text='print("new")'),
                TextAssertion(path="src/utils.py", kind="not_contains", text='print("old")'),
            ),
        ),
        score_profile="apply",
    ),
    EditBenchmarkTask(
        id="apply_02",
        title="Apply directory-scoped replacement",
        phase="phase2",
        mode="apply",
        category="directory_scoped_rewrite",
        language="python",
        scope="multi_file",
        instruction="Apply a replacement of print('old') with print('new') across src",
        operation=EditOperation(
            name="pattern_rewrite",
            path="src",
            pattern='print("old")',
            replacement='print("new")',
            language="python",
            dry_run=False,
        ),
        constraints=EditConstraints(allow_apply=True, require_dry_run=True),
        expected=EditExpected(
            primary_target_file="src/secondary.py",
            apply_files=("src/secondary.py", "src/utils.py"),
            expected_change_count=2,
            verify_labels=("syntax",),
            post_apply_assertions=(
                TextAssertion(path="src/secondary.py", kind="contains", text='print("new")'),
                TextAssertion(path="src/utils.py", kind="contains", text='print("new")'),
            ),
        ),
        score_profile="apply",
    ),
)


def _task_args(task: EditBenchmarkTask) -> dict[str, object]:
    operation = task.operation
    args = {
        "operation": operation.name,
        "goal": operation.goal,
        "edit_kind": operation.edit_kind,
        "symbol_name": operation.symbol_name,
        "file_path": operation.file_path,
        "path": operation.path,
        "pattern": operation.pattern,
        "replacement": operation.replacement,
        "language": operation.language,
    }
    if operation.dry_run is not None:
        args["dry_run"] = operation.dry_run
    return {key: value for key, value in args.items() if value not in ("", None)}


def _normalize_run(task: EditBenchmarkTask, result: dict, codebase_root: Path) -> EditBenchmarkRun:
    run = EditBenchmarkRun(task_id=task.id)
    run.tokens = estimate_tokens(result)

    result_files = [item.get("file") for item in result.get("results", []) if item.get("file")]
    primary_file = result.get("file")
    if primary_file:
        ranked_files = [primary_file]
    else:
        ranked_files = list(result_files)

    if task.score_profile == "plan_only":
        run.ranked_target_files = list(result.get("target_files", []))
        run.predicted_impacted_files = list(result.get("impact", {}).get("impacted_files", []))
        run.predicted_verify_labels = list(result.get("verify", {}).get("labels", []))
        run.predicted_risk_labels = list(result.get("risks", []))
        run.rollback_present = bool(result.get("rollback"))
        return run

    if task.score_profile == "multi_file_plan":
        run.ranked_target_files = list(result.get("target_files", []))
        run.predicted_impacted_files = list(result.get("impact", {}).get("impacted_files", []))
        run.predicted_verify_labels = list(result.get("verify", {}).get("labels", []))
        run.predicted_risk_labels = list(result.get("risks", []))
        run.rollback_present = bool(result.get("rollback"))
        return run

    if task.score_profile == "preview":
        run.ranked_target_files = ranked_files
        run.preview_files = ranked_files
        run.preview_change_count = int(result.get("changes", result.get("total_changes", 0)))
        run.predicted_verify_labels = ["syntax"]
        passed, total = evaluate_text_assertions(
            codebase_root,
            task.expected.preview_assertions,
            preview_text=result.get("diff", "")
            or "\n".join(item.get("diff", "") for item in result.get("results", [])),
        )
        run.passed_preview_assertions = passed
        run.total_preview_assertions = total
        return run

    if task.score_profile == "apply":
        run.ranked_target_files = ranked_files
        run.apply_files = ranked_files
        run.apply_change_count = int(result.get("changes", result.get("total_changes", 0)))
        run.predicted_verify_labels = ["syntax"]
        passed, total = evaluate_text_assertions(codebase_root, task.expected.post_apply_assertions)
        run.passed_post_apply_assertions = passed
        run.total_post_apply_assertions = total
        run.syntax_passed = passed == total
        return run

    return run


async def run_benchmark() -> dict[str, object]:
    """Run the edit-assistance benchmark suite."""
    task_results = []
    for task in TASKS:
        tmp_dir = Path(tempfile.mkdtemp(prefix=f"ctx_edit_bench_{task.id}_"))
        storage_dir = tmp_dir / ".contextro"
        storage_dir.mkdir()
        codebase = create_edit_benchmark_codebase(tmp_dir)

        with benchmark_session(
            storage_dir,
            embedding_model="bge-small-en",
            dims=384,
            env_overrides={"CTX_EDIT_REQUIRE_PREVIEW_BEFORE_APPLY": "true"},
        ) as (mcp, _mock_svc, server_module):
            await index_codebase(mcp, server_module, str(codebase))

            preview_result = None
            start = time.perf_counter()
            if task.score_profile == "apply":
                preview_task_id = "preview_01" if task.id == "apply_01" else "preview_02"
                preview_task = next(item for item in TASKS if item.id == preview_task_id)
                preview_result = await call_tool(mcp, "code", _task_args(preview_task))

            result = await call_tool(mcp, "code", _task_args(task))
            latency_ms = round((time.perf_counter() - start) * 1000, 2)
            run = _normalize_run(task, result, codebase)
            run.latency_ms = latency_ms
            run.tool_calls = 1 if task.score_profile != "apply" else 2
            if task.score_profile == "apply" and preview_result is not None:
                preview_files = [
                    item.get("file")
                    for item in preview_result.get("results", [])
                    if item.get("file")
                ]
                if preview_result.get("file"):
                    preview_files = [preview_result["file"]]
                run.preview_files = preview_files
                run.preview_change_count = int(
                    preview_result.get("changes", preview_result.get("total_changes", 0))
                )

            score = score_run(task, run)
            task_results.append(
                {
                    "task": asdict(task),
                    "result": result,
                    "run": asdict(run),
                    "score": score,
                }
            )

    passed = sum(1 for item in task_results if item["score"]["passed"])
    summary = {
        "tasks": len(task_results),
        "passed": passed,
        "failed": len(task_results) - passed,
        "mean_score": round(
            sum(item["score"]["score"] for item in task_results) / max(len(task_results), 1),
            1,
        ),
    }
    return {"summary": summary, "results": task_results}


def main() -> None:
    metrics = asyncio.run(run_benchmark())
    output_path = Path(__file__).with_name("edit_benchmark_results.json")
    output_path.write_text(json.dumps(metrics, indent=2) + "\n")
    print(json.dumps(metrics["summary"], indent=2))
    print(f"Results saved to: {output_path}")


if __name__ == "__main__":
    main()
