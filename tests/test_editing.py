"""Tests for edit assistance planning, preview, apply, and scoring."""

import asyncio
from pathlib import Path

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
from tests.conftest import _call_tool, _setup_indexed


def _create_edit_codebase(root: Path) -> Path:
    src = root / "src"
    tests = root / "tests"
    src.mkdir()
    tests.mkdir()

    (src / "utils.py").write_text(
        "def helper():\n"
        '    print("old")\n\n'
        "def unused():\n"
        "    return 0\n"
    )
    (src / "main.py").write_text(
        "from src.utils import helper\n\n\n"
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
    return root


class TestEditAssistance:
    def test_edit_plan_returns_targets_risks_and_verify_steps(self, tmp_path):
        codebase = _create_edit_codebase(tmp_path)

        async def run():
            mcp, _, _ = await _setup_indexed(codebase, tmp_path / ".contextro")
            return await _call_tool(
                mcp,
                "code",
                {
                    "operation": "edit_plan",
                    "goal": "Rename helper logging call",
                    "symbol_name": "helper",
                    "file_path": "src/utils.py",
                    "pattern": 'print("old")',
                    "replacement": 'print("new")',
                    "language": "python",
                },
            )

        result = asyncio.run(run())

        assert result["operation"] == "edit_plan"
        assert result["primary_target_file"] == "src/utils.py"
        assert "src/utils.py" in result["target_files"]
        assert result["verify"]["labels"][0] == "syntax"
        assert result["recommended_operation"] == "pattern_rewrite"
        assert result["requires_preview"] is True
        assert 0.0 < result["confidence"] <= 1.0

    def test_edit_plan_prefers_pattern_matches_for_directory_scope(self, tmp_path):
        codebase = _create_edit_codebase(tmp_path)

        async def run():
            mcp, _, _ = await _setup_indexed(codebase, tmp_path / ".contextro")
            return await _call_tool(
                mcp,
                "code",
                {
                    "operation": "edit_plan",
                    "goal": "Replace print calls across src",
                    "path": "src",
                    "pattern": 'print("old")',
                    "replacement": 'print("new")',
                    "language": "python",
                },
            )

        result = asyncio.run(run())

        assert result["scope"] == "directory"
        assert result["primary_target_file"] == "src/secondary.py"
        assert result["target_files"][:2] == ["src/secondary.py", "src/utils.py"]
        assert "src/main.py" not in result["target_files"]
        assert "broad_rewrite" in result["risks"]

    def test_pattern_rewrite_preview_returns_diff_and_changed_symbols(self, tmp_path):
        codebase = _create_edit_codebase(tmp_path)

        async def run():
            mcp, _, _ = await _setup_indexed(codebase, tmp_path / ".contextro")
            return await _call_tool(
                mcp,
                "code",
                {
                    "operation": "pattern_rewrite",
                    "pattern": 'print("old")',
                    "replacement": 'print("new")',
                    "language": "python",
                    "file_path": "src/utils.py",
                    "dry_run": True,
                },
            )

        result = asyncio.run(run())

        assert result["operation"] == "pattern_rewrite"
        assert result["dry_run"] is True
        assert result["changes"] == 1
        assert "@@" in result["diff"]
        assert result["preview_signature"]
        assert result["changed_symbols"][0]["name"] == "helper"

    def test_pattern_rewrite_directory_scope_previews_all_matching_files(self, tmp_path):
        codebase = _create_edit_codebase(tmp_path)

        async def run():
            mcp, _, _ = await _setup_indexed(codebase, tmp_path / ".contextro")
            return await _call_tool(
                mcp,
                "code",
                {
                    "operation": "pattern_rewrite",
                    "pattern": 'print("old")',
                    "replacement": 'print("new")',
                    "language": "python",
                    "path": "src",
                    "dry_run": True,
                },
            )

        result = asyncio.run(run())

        assert result["operation"] == "pattern_rewrite"
        assert result["dry_run"] is True
        assert result["files_modified"] == 2
        assert result["total_changes"] == 2
        assert [item["file"] for item in result["results"]] == [
            "src/secondary.py",
            "src/utils.py",
        ]

    def test_pattern_rewrite_path_can_target_a_single_file(self, tmp_path):
        codebase = _create_edit_codebase(tmp_path)

        async def run():
            mcp, _, _ = await _setup_indexed(codebase, tmp_path / ".contextro")
            return await _call_tool(
                mcp,
                "code",
                {
                    "operation": "pattern_rewrite",
                    "pattern": 'print("old")',
                    "replacement": 'print("new")',
                    "language": "python",
                    "path": "src/secondary.py",
                    "dry_run": True,
                },
            )

        result = asyncio.run(run())

        assert result["files_modified"] == 1
        assert result["results"][0]["file"] == "src/secondary.py"

    def test_pattern_rewrite_reports_no_match_without_error(self, tmp_path):
        codebase = _create_edit_codebase(tmp_path)

        async def run():
            mcp, _, _ = await _setup_indexed(codebase, tmp_path / ".contextro")
            return await _call_tool(
                mcp,
                "code",
                {
                    "operation": "pattern_rewrite",
                    "pattern": 'print("missing")',
                    "replacement": 'print("new")',
                    "language": "python",
                    "file_path": "src/utils.py",
                    "dry_run": True,
                },
            )

        result = asyncio.run(run())

        assert "error" not in result
        assert result["changes"] == 0
        assert result["message"] == "No matching files found"

    def test_pattern_rewrite_apply_can_require_recent_preview(self, tmp_path, monkeypatch):
        codebase = _create_edit_codebase(tmp_path)
        monkeypatch.setenv("CTX_EDIT_REQUIRE_PREVIEW_BEFORE_APPLY", "true")

        async def run_without_preview():
            mcp, _, _ = await _setup_indexed(codebase, tmp_path / ".contextro")
            return await _call_tool(
                mcp,
                "code",
                {
                    "operation": "pattern_rewrite",
                    "pattern": 'print("old")',
                    "replacement": 'print("new")',
                    "language": "python",
                    "file_path": "src/utils.py",
                    "dry_run": False,
                },
            )

        result = asyncio.run(run_without_preview())

        assert result["preview_required"] is True
        assert "Preview required" in result["error"]

    def test_pattern_rewrite_apply_updates_file_after_preview(self, tmp_path, monkeypatch):
        codebase = _create_edit_codebase(tmp_path)
        monkeypatch.setenv("CTX_EDIT_REQUIRE_PREVIEW_BEFORE_APPLY", "true")

        async def run():
            mcp, _, _ = await _setup_indexed(codebase, tmp_path / ".contextro")
            preview = await _call_tool(
                mcp,
                "code",
                {
                    "operation": "pattern_rewrite",
                    "pattern": 'print("old")',
                    "replacement": 'print("new")',
                    "language": "python",
                    "file_path": "src/utils.py",
                    "dry_run": True,
                },
            )
            apply_result = await _call_tool(
                mcp,
                "code",
                {
                    "operation": "pattern_rewrite",
                    "pattern": 'print("old")',
                    "replacement": 'print("new")',
                    "language": "python",
                    "file_path": "src/utils.py",
                    "dry_run": False,
                },
            )
            snapshot = await _call_tool(mcp, "session_snapshot")
            return preview, apply_result, snapshot

        preview, apply_result, snapshot = asyncio.run(run())

        assert preview["preview_signature"] == apply_result["preview_signature"]
        assert apply_result["applied"] is True
        assert "edit_metrics" in snapshot
        assert snapshot["edit_metrics"]["previews"] >= 1
        assert snapshot["edit_metrics"]["applies"] >= 1
        assert 'print("new")' in (tmp_path / "src" / "utils.py").read_text()

    def test_status_surfaces_edit_metrics_after_preview(self, tmp_path):
        codebase = _create_edit_codebase(tmp_path)

        async def run():
            mcp, _, _ = await _setup_indexed(codebase, tmp_path / ".contextro")
            await _call_tool(
                mcp,
                "code",
                {
                    "operation": "pattern_rewrite",
                    "pattern": 'print("old")',
                    "replacement": 'print("new")',
                    "language": "python",
                    "file_path": "src/utils.py",
                    "dry_run": True,
                },
            )
            return await _call_tool(mcp, "status")

        status = asyncio.run(run())

        assert status["edit"]["previews"] >= 1


def test_evaluate_text_assertions_supports_preview_and_file_checks(tmp_path):
    target = tmp_path / "demo.py"
    target.write_text("print('new')\n")

    assertions = (
        TextAssertion(source="preview", kind="contains", text="@@"),
        TextAssertion(path="demo.py", kind="contains", text="new"),
        TextAssertion(path="demo.py", kind="not_contains", text="old"),
    )

    preview_text = "@@\n-print\n+print"
    passed, total = evaluate_text_assertions(tmp_path, assertions, preview_text=preview_text)
    assert (passed, total) == (3, 3)


def test_score_run_uses_profile_thresholds_and_invariants():
    task = EditBenchmarkTask(
        id="preview_01",
        title="Preview rename",
        phase="phase2",
        mode="preview",
        category="single_file_call_replacement",
        language="python",
        scope="single_file",
        instruction="Preview a simple call replacement",
        operation=EditOperation(name="pattern_rewrite", language="python"),
        constraints=EditConstraints(require_dry_run=True),
        expected=EditExpected(
            primary_target_file="src/utils.py",
            preview_files=("src/utils.py",),
            expected_change_count=1,
            verify_labels=("syntax",),
            preview_assertions=(TextAssertion(source="preview", kind="contains", text="@@"),),
        ),
        score_profile="preview",
    )
    run = EditBenchmarkRun(
        task_id="preview_01",
        ranked_target_files=["src/utils.py"],
        preview_files=["src/utils.py"],
        preview_change_count=1,
        predicted_verify_labels=["syntax"],
        passed_preview_assertions=1,
        total_preview_assertions=1,
    )

    scored = score_run(task, run)
    assert scored["passed"] is True
    assert scored["score"] >= scored["threshold"]

    run.invariant_violations.append("missing_required_dry_run")
    scored = score_run(task, run)
    assert scored["passed"] is False
