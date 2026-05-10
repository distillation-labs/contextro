"""Edit-planning helpers for scoped, preview-first code changes."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from contextro_mcp.analysis.repository_map import related_tests as find_related_tests
from contextro_mcp.reports.product import get_repository_map_for_state

_LANGUAGE_EXTENSIONS = {
    "python": (".py",),
    "javascript": (".js", ".jsx", ".mjs", ".cjs"),
    "typescript": (".ts", ".tsx"),
    "rust": (".rs",),
    "go": (".go",),
    "java": (".java",),
    "cpp": (".cpp", ".cc", ".cxx", ".h", ".hpp"),
    "c": (".c", ".h"),
    "ruby": (".rb",),
    "php": (".php",),
    "swift": (".swift",),
    "kotlin": (".kt",),
    "csharp": (".cs",),
}


def _language_extensions(language: str) -> tuple[str, ...]:
    if not language:
        return ()
    return _LANGUAGE_EXTENSIONS.get(language.lower(), (f".{language.lower()}",))


def _relativize(root: Path, raw_path: str) -> str | None:
    if not raw_path:
        return None
    candidate = Path(raw_path)
    if not candidate.is_absolute():
        candidate = root / raw_path
    candidate = candidate.resolve(strict=False)
    try:
        return candidate.relative_to(root).as_posix()
    except ValueError:
        return None


def _resolve_scope_files(
    root: Path,
    repo_map,
    *,
    file_path: str,
    path: str,
    language: str,
    max_targets: int,
) -> list[str]:
    if file_path:
        relative = _relativize(root, file_path)
        if relative and relative in repo_map.modules:
            return [relative]
        return []

    if not path:
        return []

    relative = _relativize(root, path)
    if relative and relative in repo_map.modules:
        return [relative]

    dir_path = Path(path)
    if not dir_path.is_absolute():
        dir_path = (root / path).resolve(strict=False)
    else:
        dir_path = dir_path.resolve(strict=False)
    try:
        relative_dir = dir_path.relative_to(root).as_posix().rstrip("/")
    except ValueError:
        return []

    extensions = _language_extensions(language)
    matches = []
    prefix = relative_dir + "/"
    for module_path in sorted(repo_map.modules):
        if module_path == relative_dir or module_path.startswith(prefix):
            if extensions and not module_path.endswith(extensions):
                continue
            matches.append(module_path)
    return matches[:max_targets]


def _pattern_target_files(
    root: Path,
    repo_map,
    *,
    pattern: str,
    language: str,
    scope_files: list[str],
    max_targets: int,
) -> list[str]:
    if not pattern or not language:
        return []
    try:
        from ast_grep_py import SgRoot
    except ImportError:
        return []

    extensions = _language_extensions(language)
    candidates = scope_files or [
        module_path
        for module_path in sorted(repo_map.modules)
        if not extensions or module_path.endswith(extensions)
    ]
    matches: list[str] = []
    for relative_path in candidates:
        source = (root / relative_path).read_text(errors="replace")
        try:
            root_node = SgRoot(source, language.lower())
        except Exception:
            continue
        if root_node.root().find_all(pattern=pattern):
            matches.append(relative_path)
        if len(matches) >= max_targets:
            break
    return matches


def _symbol_matches(state, symbol_name: str, path_prefix: str, limit: int):
    if not symbol_name:
        return [], False
    graph = state.graph_engine
    exact = graph.find_nodes_by_name(symbol_name, exact=True)
    fuzzy = [] if exact else graph.find_nodes_by_name(symbol_name, exact=False)
    matches = exact or fuzzy
    if path_prefix:
        matches = [
            node
            for node in matches
            if path_prefix in str(node.location.file_path).replace("\\", "/")
        ]
    return matches[:limit], bool(exact)


def _symbol_strings(nodes, root: Path) -> list[str]:
    results = []
    for node in nodes:
        try:
            file_path = (
                Path(node.location.file_path)
                .resolve(strict=False)
                .relative_to(root)
                .as_posix()
            )
        except ValueError:
            file_path = str(node.location.file_path).replace("\\", "/")
        results.append(f"{node.name} ({file_path}:{node.location.start_line})")
    return results


def _impacted_files(state, nodes, root: Path) -> list[str]:
    files: list[str] = []
    seen: set[str] = set()
    for node in nodes:
        for caller in state.graph_engine.get_transitive_callers(node.id, max_depth=10):
            try:
                rel_path = (
                    Path(caller.location.file_path).resolve(strict=False).relative_to(root).as_posix()
                )
            except ValueError:
                continue
            if rel_path not in seen:
                seen.add(rel_path)
                files.append(rel_path)
    return files


def _verify_steps(target_files: list[str], related_tests: list[str]) -> tuple[list[str], list[str]]:
    labels: list[str] = ["syntax"]
    commands: list[str] = []
    python_files = [path for path in target_files if path.endswith(".py")]
    if python_files:
        commands.append("python -m py_compile " + " ".join(python_files[:5]))
        labels.append("lint")
        commands.append("ruff check " + " ".join(python_files[:5]))
    if related_tests:
        labels.append("unit_tests")
        commands.append("pytest -q " + " ".join(related_tests[:5]))
    if not commands:
        commands.append("Run project-specific syntax, lint, and targeted tests")
    return labels, commands


def _infer_edit_kind(
    edit_kind: str,
    goal: str,
    symbol_name: str,
    pattern: str,
    replacement: str,
    target_files: list[str],
) -> str:
    if edit_kind:
        return edit_kind
    goal_lower = goal.lower()
    if "rename" in goal_lower:
        return "rename"
    if "signature" in goal_lower or "parameter" in goal_lower:
        return "signature"
    if pattern and replacement and len(target_files) > 1:
        return "multi_file"
    if pattern and replacement:
        return "replace"
    if symbol_name:
        return "refactor"
    return "manual"


def _scope_label(file_path: str, path: str, target_files: list[str]) -> str:
    if file_path or len(target_files) == 1:
        return "single_file"
    if path:
        return "directory"
    if len(target_files) > 1:
        return "multi_file"
    return "unknown"


def build_edit_plan(
    state,
    *,
    goal: str = "",
    edit_kind: str = "",
    symbol_name: str = "",
    file_path: str = "",
    path: str = "",
    pattern: str = "",
    replacement: str = "",
    language: str = "",
    limit: int = 5,
) -> dict[str, Any]:
    """Build a repo-aware edit plan without mutating files."""
    if state.codebase_path is None:
        raise ValueError("No codebase indexed. Run index first.")

    root = state.codebase_path.resolve()
    repo_map = get_repository_map_for_state(state)
    max_targets = max(1, min(limit, 12))

    explicit_scope = _resolve_scope_files(
        root,
        repo_map,
        file_path=file_path,
        path=path,
        language=language,
        max_targets=max_targets,
    )

    path_prefix = file_path or path
    symbol_nodes, exact_symbol_match = _symbol_matches(state, symbol_name, path_prefix, max_targets)
    symbol_files = []
    for node in symbol_nodes:
        try:
            rel_path = (
                Path(node.location.file_path)
                .resolve(strict=False)
                .relative_to(root)
                .as_posix()
            )
        except ValueError:
            continue
        if rel_path not in symbol_files:
            symbol_files.append(rel_path)

    pattern_files = _pattern_target_files(
        root,
        repo_map,
        pattern=pattern,
        language=language,
        scope_files=explicit_scope,
        max_targets=max_targets,
    )

    target_files: list[str] = []
    if pattern_files:
        buckets = (pattern_files, symbol_files)
        if file_path:
            buckets = (pattern_files, symbol_files, explicit_scope)
    else:
        buckets = (explicit_scope, symbol_files, pattern_files)

    for bucket in buckets:
        for candidate in bucket:
            if candidate not in target_files:
                target_files.append(candidate)
            if len(target_files) >= max_targets:
                break
        if len(target_files) >= max_targets:
            break

    primary_target_file = target_files[0] if target_files else None
    nearby_tests: list[str] = []
    for target_file in target_files[:5]:
        for test_file in find_related_tests(repo_map, target_file):
            if test_file not in nearby_tests:
                nearby_tests.append(test_file)

    impacted_files = _impacted_files(state, symbol_nodes[:3], root)
    apply_sequence = []
    if primary_target_file:
        apply_sequence.append(primary_target_file)
    for target_file in target_files:
        if target_file != primary_target_file:
            apply_sequence.append(target_file)
    for impacted_file in impacted_files:
        if impacted_file not in apply_sequence and impacted_file in target_files:
            apply_sequence.append(impacted_file)

    inferred_kind = _infer_edit_kind(
        edit_kind, goal, symbol_name, pattern, replacement, target_files
    )
    scope = _scope_label(file_path, path, target_files)
    verify_labels, verify_commands = _verify_steps(target_files or impacted_files, nearby_tests)

    risks: list[str] = []
    if not primary_target_file:
        risks.append("wrong_target")
    if len(target_files) > 3 or scope == "directory":
        risks.append("broad_rewrite")
    if impacted_files:
        risks.append("api_break")
    if not nearby_tests:
        risks.append("test_break")

    confidence = 0.2
    if explicit_scope:
        confidence += 0.35
    if primary_target_file:
        confidence += 0.15
    if exact_symbol_match:
        confidence += 0.25
    elif symbol_nodes:
        confidence += 0.1
    if pattern_files:
        confidence += 0.15
    if len(target_files) > 3:
        confidence -= 0.1
    if not nearby_tests:
        confidence -= 0.05
    confidence = round(max(0.05, min(confidence, 0.99)), 2)

    goal_text = goal or symbol_name or f"Rewrite {pattern!r}"
    recommended_operation = "pattern_rewrite" if pattern and replacement else "manual_review"
    rollback_targets = target_files or impacted_files
    rollback = "Review git diff and restore touched files if needed."
    if rollback_targets:
        rollback += " Candidate restore set: git restore -- " + " ".join(rollback_targets[:8])

    return {
        "operation": "edit_plan",
        "goal": goal_text,
        "edit_kind": inferred_kind,
        "scope": scope,
        "primary_target_file": primary_target_file,
        "target_files": target_files,
        "target_symbols": _symbol_strings(symbol_nodes[:max_targets], root),
        "impact": {
            "required": inferred_kind in {"rename", "signature", "refactor"} and bool(symbol_name),
            "impacted_files": impacted_files[:12],
            "total_impacted": len(impacted_files),
        },
        "related_tests": nearby_tests[:12],
        "verify": {
            "labels": verify_labels,
            "commands": verify_commands,
        },
        "risks": risks,
        "allowed_paths": target_files,
        "recommended_operation": recommended_operation,
        "requires_preview": True,
        "apply_sequence": apply_sequence,
        "rollback": rollback,
        "confidence": confidence,
    }
