"""Edit assistance helpers for planning, previewing, and benchmarking edits."""

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
from contextro_mcp.editing.planner import build_edit_plan
from contextro_mcp.editing.rewrite import (
    build_rewrite_signature,
    execute_pattern_rewrite,
    has_fresh_preview,
    remember_preview,
)

__all__ = [
    "EditBenchmarkRun",
    "EditBenchmarkTask",
    "EditConstraints",
    "EditExpected",
    "EditOperation",
    "TextAssertion",
    "build_edit_plan",
    "build_rewrite_signature",
    "evaluate_text_assertions",
    "execute_pattern_rewrite",
    "has_fresh_preview",
    "remember_preview",
    "score_run",
]
