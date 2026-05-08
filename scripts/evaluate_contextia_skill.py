#!/usr/bin/env python3
import json
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SKILLS_ROOT = ROOT / ".kiro/skills"


def discover_skills() -> list[str]:
    if not SKILLS_ROOT.exists():
        return []
    return sorted(
        path.name
        for path in SKILLS_ROOT.iterdir()
        if path.is_dir() and (path / "SKILL.md").exists()
    )


DEFAULT_SKILLS = discover_skills()


@dataclass(frozen=True)
class CheckResult:
    name: str
    passed: bool
    detail: str


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def load_json(path: Path) -> dict:
    return json.loads(read_text(path))


def parse_frontmatter(text: str) -> str:
    parts = text.split("---", 2)
    if len(parts) < 3:
        raise ValueError("SKILL.md missing frontmatter delimiters")
    return parts[1]


def collect_skill_paths(skill_name: str) -> tuple[Path, Path]:
    skill_dir = SKILLS_ROOT / skill_name
    return skill_dir / "SKILL.md", skill_dir / "evals"


def check_eval_json_files(eval_dir: Path) -> list[CheckResult]:
    if not eval_dir.exists():
        return [CheckResult("eval_dir_exists", False, "Missing evals directory.")]

    json_files = sorted(eval_dir.glob("*.json"))
    if not json_files:
        return [CheckResult("eval_json_files_exist", False, "No eval JSON files found.")]

    return [
        CheckResult("eval_dir_exists", True, f"Found eval directory at {eval_dir}."),
        CheckResult(
            "eval_json_files_exist",
            True,
            f"Found {len(json_files)} eval JSON file(s).",
        ),
    ]


def check_skill_structure(skill_text: str) -> list[CheckResult]:
    frontmatter = parse_frontmatter(skill_text)
    lines = skill_text.splitlines()
    desc_has_what = "description:" in frontmatter
    desc_has_when = "when_to_use:" in frontmatter or "use when" in frontmatter.lower()
    return [
        CheckResult("frontmatter_has_name", "name:" in frontmatter, "Frontmatter includes name."),
        CheckResult(
            "frontmatter_has_description",
            "description:" in frontmatter,
            "Frontmatter includes description.",
        ),
        CheckResult(
            "frontmatter_says_what_and_when",
            desc_has_what and desc_has_when,
            "Frontmatter includes purpose and trigger guidance.",
        ),
        CheckResult(
            "skill_is_concise",
            len(lines) <= 500,
            f"SKILL.md has {len(lines)} lines.",
        ),
        CheckResult(
            "references_are_used",
            "references/" in skill_text,
            "SKILL.md links to supporting references.",
        ),
    ]


def collect_evals(eval_dir: Path) -> list[dict]:
    evals: list[dict] = []
    if not eval_dir.exists():
        return evals
    for path in sorted(eval_dir.glob("*.json")):
        payload = load_json(path)
        for item in payload.get("evals", []):
            entry = dict(item)
            entry["_file"] = path.name
            evals.append(entry)
    return evals


def eval_categories(evals: list[dict]) -> set[str]:
    return {e.get("category") for e in evals if e.get("category")}


def dump_contains(evals: list[dict], needle: str) -> bool:
    lowered = needle.lower()
    return any(lowered in json.dumps(e).lower() for e in evals)


def unique_names(evals: list[dict]) -> bool:
    names = [e.get("name") for e in evals]
    return len(names) == len(set(names))


def base_eval_checks(evals: list[dict]) -> list[CheckResult]:
    categories = eval_categories(evals)
    return [
        CheckResult("has_enough_evals", len(evals) >= 12, f"Found {len(evals)} eval cases."),
        CheckResult(
            "has_non_trigger_cases",
            any(e.get("should_trigger") is False for e in evals),
            "At least one eval checks non-trigger behavior.",
        ),
        CheckResult(
            "all_evals_have_assertions",
            all(e.get("assertions") for e in evals),
            "Every eval has assertions.",
        ),
        CheckResult("eval_names_are_unique", unique_names(evals), "Eval names are unique."),
        CheckResult(
            "covers_triggering",
            "triggering" in categories,
            f"Categories: {sorted(categories)}",
        ),
    ]


def contextro_checks(evals: list[dict]) -> list[CheckResult]:
    categories = eval_categories(evals)
    return [
        CheckResult(
            "covers_functional_or_workflow",
            bool({"functional", "workflow"} & categories),
            f"Categories: {sorted(categories)}",
        ),
        CheckResult(
            "covers_performance",
            "performance_comparison" in categories,
            f"Categories: {sorted(categories)}",
        ),
        CheckResult(
            "covers_new_tools",
            "new_tools" in categories,
            f"Categories: {sorted(categories)}",
        ),
        CheckResult(
            "covers_refactor_safety",
            dump_contains(evals, "impact"),
            "At least one eval checks impact-before-refactor behavior.",
        ),
        CheckResult(
            "covers_sandbox_retrieve",
            dump_contains(evals, "sandbox_ref") or dump_contains(evals, "retrieve"),
            "At least one eval checks retrieve handling.",
        ),
        CheckResult(
            "covers_compaction_recovery",
            dump_contains(evals, "session_snapshot"),
            "At least one eval checks post-compaction recovery.",
        ),
        CheckResult(
            "covers_ast_rewrite_safety",
            dump_contains(evals, "dry_run"),
            "At least one eval checks AST rewrite dry-run discipline.",
        ),
        CheckResult(
            "covers_direct_file_non_trigger",
            dump_contains(evals, "pyproject.toml") or dump_contains(evals, "single-file"),
            "At least one eval checks non-trigger behavior for direct file reads.",
        ),
    ]


def breakthrough_checks(evals: list[dict]) -> list[CheckResult]:
    categories = eval_categories(evals)
    return [
        CheckResult(
            "covers_methodology",
            "methodology" in categories,
            f"Categories: {sorted(categories)}",
        ),
        CheckResult(
            "covers_research_workflow",
            "workflow" in categories,
            f"Categories: {sorted(categories)}",
        ),
        CheckResult(
            "requires_fact_inference_hypothesis_split",
            dump_contains(evals, "fact") and dump_contains(evals, "hypothesis"),
            "Evals check explicit evidence handling.",
        ),
        CheckResult(
            "requires_falsifiable_experiments",
            dump_contains(evals, "success criteria")
            or dump_contains(evals, "baseline, and success threshold"),
            "Evals check for measurable experiment output.",
        ),
        CheckResult(
            "rejects_cargo_culting",
            dump_contains(evals, "copy") or dump_contains(evals, "cargo"),
            "Evals check anti-cargo-cult behavior.",
        ),
    ]


def applied_checks(evals: list[dict]) -> list[CheckResult]:
    categories = eval_categories(evals)
    return [
        CheckResult(
            "covers_harness_or_eval_design",
            "evals" in categories,
            f"Categories: {sorted(categories)}",
        ),
        CheckResult(
            "covers_observability",
            "observability" in categories,
            f"Categories: {sorted(categories)}",
        ),
        CheckResult(
            "requires_baseline_and_metric",
            dump_contains(evals, "baseline") and dump_contains(evals, "metric"),
            "Evals check for metric and baseline discipline.",
        ),
        CheckResult(
            "rejects_ship_without_evals",
            dump_contains(evals, "without eval") or dump_contains(evals, "without evaluation"),
            "Evals check anti-pattern against shipping without validation.",
        ),
        CheckResult(
            "requires_rollout_or_rollback",
            dump_contains(evals, "rollback") or dump_contains(evals, "rollout"),
            "Evals check deployment safety planning.",
        ),
    ]


def generic_skill_checks(skill_name: str, skill_text: str, evals: list[dict]) -> list[CheckResult]:
    lowered_skill = skill_text.lower()
    lower_dump = json.dumps(evals).lower()
    domain_tokens = [token for token in skill_name.replace("-", " ").split() if len(token) > 2]
    domain_hits = [
        token for token in domain_tokens if token in lowered_skill or token in lower_dump
    ]
    return [
        CheckResult(
            "skill_mentions_domain_terms",
            bool(domain_hits),
            f"Domain terms referenced: {domain_hits or 'none'}.",
        ),
        CheckResult(
            "evals_reference_repo_or_tooling",
            dump_contains(evals, "pytest")
            or dump_contains(evals, "ruff")
            or dump_contains(evals, "benchmark")
            or dump_contains(evals, "contextro")
            or dump_contains(evals, "mcp"),
            "Evals reference repository workflows, tooling, or MCP-specific behavior.",
        ),
    ]


def summarize(results: list[CheckResult]) -> dict:
    passed = sum(1 for r in results if r.passed)
    total = len(results)
    return {
        "passed": passed,
        "total": total,
        "score": round(passed / total, 3) if total else 0,
        "checks": [{"name": r.name, "passed": r.passed, "detail": r.detail} for r in results],
    }


def evaluate_skill(skill_name: str) -> dict:
    skill_path, eval_dir = collect_skill_paths(skill_name)
    skill_text = read_text(skill_path)
    evals = collect_evals(eval_dir)
    results = [
        *check_skill_structure(skill_text),
        *check_eval_json_files(eval_dir),
        *base_eval_checks(evals),
    ]
    if skill_name == "dev-contextro-mcp":
        results.extend(contextro_checks(evals))
    elif skill_name == "breakthrough-researcher":
        results.extend(breakthrough_checks(evals))
    elif skill_name == "applied-ai-engineer":
        results.extend(applied_checks(evals))
    else:
        results.extend(generic_skill_checks(skill_name, skill_text, evals))
    summary = summarize(results)
    summary["skill"] = skill_name
    summary["skill_lines"] = len(skill_text.splitlines())
    summary["eval_count"] = len(evals)
    return summary


def main() -> None:
    skills = sys.argv[1:] or DEFAULT_SKILLS
    output = {skill: evaluate_skill(skill) for skill in skills}
    print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
