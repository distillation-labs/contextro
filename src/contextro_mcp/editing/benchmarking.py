"""Benchmark schema and scoring helpers for edit-assistance evaluation."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Literal

AssertionKind = Literal["contains", "not_contains"]
AssertionSource = Literal["file", "preview"]
ScoreProfile = Literal["plan_only", "preview", "apply", "multi_file_plan"]


@dataclass(frozen=True)
class TextAssertion:
    """A deterministic text assertion for preview or post-apply evaluation."""

    kind: AssertionKind
    text: str
    source: AssertionSource = "file"
    path: str = ""


@dataclass(frozen=True)
class EditOperation:
    """Normalized operation payload for edit-assistance benchmarks."""

    tool: str = "code"
    name: str = "edit_plan"
    symbol_name: str = ""
    file_path: str = ""
    path: str = ""
    pattern: str = ""
    replacement: str = ""
    language: str = ""
    goal: str = ""
    edit_kind: str = ""
    dry_run: bool | None = None


@dataclass(frozen=True)
class EditConstraints:
    """Safety and workflow constraints for a benchmark task."""

    allow_apply: bool = False
    require_dry_run: bool = False
    require_impact: bool = False
    allowed_paths: tuple[str, ...] = ()
    forbidden_paths: tuple[str, ...] = ()
    must_use_relative_paths: bool = True


@dataclass(frozen=True)
class EditExpected:
    """Expected outputs and invariants for a benchmark task."""

    primary_target_file: str | None = None
    target_files: tuple[str, ...] = ()
    target_symbols: tuple[str, ...] = ()
    impacted_files: tuple[str, ...] = ()
    order_constraints: tuple[tuple[str, str], ...] = ()
    preview_files: tuple[str, ...] = ()
    apply_files: tuple[str, ...] = ()
    expected_change_count: int | None = None
    verify_labels: tuple[str, ...] = ()
    acceptable_commands: tuple[str, ...] = ()
    risk_labels: tuple[str, ...] = ()
    must_not_touch: tuple[str, ...] = ()
    preview_assertions: tuple[TextAssertion, ...] = ()
    post_apply_assertions: tuple[TextAssertion, ...] = ()


@dataclass(frozen=True)
class EditBenchmarkTask:
    """Full schema for one edit-assistance benchmark task."""

    id: str
    title: str
    phase: str
    mode: str
    category: str
    language: str
    scope: str
    instruction: str
    control_grep_patterns: tuple[str, ...] = ()
    control_seed_paths: tuple[str, ...] = ()
    max_control_reads: int = 0
    operation: EditOperation = field(default_factory=EditOperation)
    constraints: EditConstraints = field(default_factory=EditConstraints)
    expected: EditExpected = field(default_factory=EditExpected)
    score_profile: ScoreProfile = "plan_only"


@dataclass
class EditBenchmarkRun:
    """Normalized measured output for one benchmark run."""

    task_id: str
    arm: str = "mcp"
    ranked_target_files: list[str] = field(default_factory=list)
    predicted_impacted_files: list[str] = field(default_factory=list)
    predicted_verify_labels: list[str] = field(default_factory=list)
    predicted_risk_labels: list[str] = field(default_factory=list)
    preview_files: list[str] = field(default_factory=list)
    preview_change_count: int | None = None
    apply_files: list[str] = field(default_factory=list)
    apply_change_count: int | None = None
    passed_preview_assertions: int = 0
    total_preview_assertions: int = 0
    passed_post_apply_assertions: int = 0
    total_post_apply_assertions: int = 0
    rollback_present: bool = False
    syntax_passed: bool | None = None
    invariant_violations: list[str] = field(default_factory=list)
    tokens: int = 0
    latency_ms: float = 0.0
    tool_calls: int = 0
    file_reads: int = 0
    wasted_reads: int = 0


_WEIGHTS: dict[ScoreProfile, dict[str, int]] = {
    "plan_only": {
        "target_rank": 30,
        "target_file_set": 25,
        "impact_file_set": 20,
        "verify_label": 10,
        "risk_label": 10,
        "rollback_present": 5,
    },
    "preview": {
        "target_rank": 20,
        "preview_file_set": 20,
        "change_count": 20,
        "preview_assertion": 30,
        "verify_label": 10,
    },
    "apply": {
        "target_rank": 15,
        "apply_file_set": 15,
        "change_count": 10,
        "apply_assertion": 25,
        "verify_label": 10,
        "parity": 10,
        "syntax": 15,
    },
    "multi_file_plan": {
        "target_file_set": 20,
        "impact_file_set": 20,
        "verify_label": 15,
        "risk_label": 15,
        "rollback_present": 10,
        "order": 20,
    },
}

_THRESHOLDS: dict[ScoreProfile, float] = {
    "plan_only": 85.0,
    "preview": 90.0,
    "apply": 95.0,
    "multi_file_plan": 85.0,
}


def rank_score(ranked_files: list[str], primary_target_file: str | None) -> float:
    """Score the rank of the primary target file."""
    if not primary_target_file:
        return 1.0
    if not ranked_files:
        return 0.0
    if ranked_files[0] == primary_target_file:
        return 1.0
    if primary_target_file in ranked_files[1:3]:
        return 0.7
    return 0.0


def set_f1(predicted: list[str] | tuple[str, ...], expected: list[str] | tuple[str, ...]) -> float:
    """Set-overlap F1 for predicted and expected file or label sets."""
    predicted_set = {item for item in predicted if item}
    expected_set = {item for item in expected if item}
    if not predicted_set and not expected_set:
        return 1.0
    if not predicted_set or not expected_set:
        return 0.0
    overlap = len(predicted_set & expected_set)
    precision = overlap / len(predicted_set)
    recall = overlap / len(expected_set)
    if precision + recall == 0:
        return 0.0
    return 2 * precision * recall / (precision + recall)


def count_score(predicted: int | None, expected: int | None) -> float:
    """Score numeric proximity for change counts."""
    if expected is None:
        return 1.0
    if predicted is None:
        return 0.0
    return max(0.0, 1.0 - abs(predicted - expected) / max(expected, 1))


def assertion_score(passed: int, total: int) -> float:
    """Fraction of deterministic assertions that passed."""
    if total <= 0:
        return 1.0
    return passed / total


def order_score(ordered_files: list[str], constraints: tuple[tuple[str, str], ...]) -> float:
    """Score whether pairwise ordering constraints are satisfied."""
    if not constraints:
        return 1.0
    positions = {path: index for index, path in enumerate(ordered_files)}
    satisfied = 0
    for before, after in constraints:
        if before in positions and after in positions and positions[before] < positions[after]:
            satisfied += 1
    return satisfied / len(constraints)


def parity_score(run: EditBenchmarkRun) -> float:
    """Score preview/apply parity across files and change counts."""
    return (
        set_f1(run.preview_files, run.apply_files)
        + count_score(run.preview_change_count, run.apply_change_count)
    ) / 2


def evaluate_text_assertions(
    root: Path,
    assertions: tuple[TextAssertion, ...] | list[TextAssertion],
    *,
    preview_text: str = "",
) -> tuple[int, int]:
    """Evaluate text assertions against preview text or repo files."""
    passed = 0
    total = len(assertions)
    for assertion in assertions:
        if assertion.source == "preview":
            haystack = preview_text
        else:
            file_path = root / assertion.path
            haystack = file_path.read_text(errors="replace") if file_path.exists() else ""
        matched = assertion.text in haystack
        if assertion.kind == "contains" and matched:
            passed += 1
        elif assertion.kind == "not_contains" and not matched:
            passed += 1
    return passed, total


def score_run(task: EditBenchmarkTask, run: EditBenchmarkRun) -> dict[str, object]:
    """Score a normalized benchmark run against a task definition."""
    profile = task.score_profile
    components = {
        "target_rank": rank_score(run.ranked_target_files, task.expected.primary_target_file),
        "target_file_set": set_f1(run.ranked_target_files, task.expected.target_files),
        "impact_file_set": set_f1(run.predicted_impacted_files, task.expected.impacted_files),
        "preview_file_set": set_f1(run.preview_files, task.expected.preview_files),
        "apply_file_set": set_f1(run.apply_files, task.expected.apply_files),
        "change_count": count_score(
            (
                run.apply_change_count
                if run.apply_change_count is not None
                else run.preview_change_count
            ),
            task.expected.expected_change_count,
        ),
        "preview_assertion": assertion_score(
            run.passed_preview_assertions, run.total_preview_assertions
        ),
        "apply_assertion": assertion_score(
            run.passed_post_apply_assertions, run.total_post_apply_assertions
        ),
        "verify_label": set_f1(run.predicted_verify_labels, task.expected.verify_labels),
        "risk_label": set_f1(run.predicted_risk_labels, task.expected.risk_labels),
        "rollback_present": 1.0 if run.rollback_present else 0.0,
        "order": order_score(run.ranked_target_files, task.expected.order_constraints),
        "parity": parity_score(run),
        "syntax": 1.0 if run.syntax_passed else 0.0,
    }

    weights = _WEIGHTS[profile]
    score = 0.0
    for name, weight in weights.items():
        score += components.get(name, 0.0) * weight

    threshold = _THRESHOLDS[profile]
    passed = score >= threshold and not run.invariant_violations
    return {
        "profile": profile,
        "score": round(score, 1),
        "threshold": threshold,
        "passed": passed,
        "components": {
            name: round(components[name], 3)
            for name in sorted(weights)
        },
        "invariant_violations": list(run.invariant_violations),
    }
