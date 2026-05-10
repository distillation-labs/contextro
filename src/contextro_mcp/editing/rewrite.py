"""Structural rewrite helpers with preview safety and parity metadata."""

from __future__ import annotations

import difflib
import hashlib
import json
import time
from pathlib import Path
from typing import Any

_LANGUAGE_EXTENSIONS = {
    "python": (".py",),
    "javascript": (".js", ".jsx", ".mjs"),
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


def _relativize(fp: Path, codebase_path: Path | None) -> str:
    if codebase_path is None:
        return str(fp)
    try:
        return str(fp.relative_to(codebase_path))
    except ValueError:
        return str(fp)


def _resolve_target_files(
    *,
    codebase_path: Path | None,
    file_path: str,
    path: str,
    language: str,
    skip_dirs: set[str] | tuple[str, ...],
) -> list[Path]:
    extensions = _language_extensions(language)
    targets: list[Path] = []
    if file_path:
        candidate = Path(file_path)
        if not candidate.is_absolute() and codebase_path is not None:
            candidate = codebase_path / file_path
        candidate = candidate.resolve(strict=False)
        if not candidate.exists():
            raise FileNotFoundError(file_path)
        return [candidate]

    candidate = Path(path)
    if not candidate.is_absolute() and codebase_path is not None:
        candidate = codebase_path / path
    candidate = candidate.resolve(strict=False)
    if not candidate.exists():
        raise NotADirectoryError(path)
    if candidate.is_file():
        return [candidate]

    for extension in extensions:
        for file_candidate in sorted(candidate.rglob(f"*{extension}")):
            if any(part in skip_dirs for part in file_candidate.parts):
                continue
            targets.append(file_candidate)
    return targets


def _diff_payload(
    source: str,
    updated: str,
    relative_path: str,
    *,
    context_lines: int,
    max_diff_lines: int,
) -> dict[str, Any]:
    diff_lines = list(
        difflib.unified_diff(
            source.splitlines(),
            updated.splitlines(),
            fromfile=relative_path,
            tofile=relative_path,
            lineterm="",
            n=context_lines,
        )
    )
    if not diff_lines:
        return {"diff": ""}
    payload: dict[str, Any] = {
        "diff": "\n".join(diff_lines[:max_diff_lines]),
        "diff_line_count": len(diff_lines),
    }
    if len(diff_lines) > max_diff_lines:
        payload["diff_truncated"] = True
        payload["full_diff"] = "\n".join(diff_lines)
    return payload


def _changed_symbols(
    graph_engine,
    fp: Path,
    touched_ranges: list[tuple[int, int]],
    codebase_path: Path | None,
):
    if graph_engine is None or not hasattr(graph_engine, "_file_nodes"):
        return []

    candidate_keys = [str(fp), str(fp.resolve(strict=False))]
    node_ids: set[str] = set()
    for key in candidate_keys:
        node_ids.update(getattr(graph_engine, "_file_nodes", {}).get(key, set()))

    nodes = []
    for node_id in node_ids:
        node = graph_engine.nodes.get(node_id)
        if node is None:
            continue
        start = node.location.start_line
        end = node.location.end_line or start
        for range_start, range_end in touched_ranges:
            if max(start, range_start) <= min(end, range_end):
                nodes.append(node)
                break

    nodes.sort(key=lambda node: (node.location.start_line, node.name))
    results = []
    for node in nodes[:8]:
        results.append(
            {
                "name": node.name,
                "type": node.node_type.value,
                "file": _relativize(Path(node.location.file_path), codebase_path),
                "line": node.location.start_line,
            }
        )
    return results


def build_rewrite_signature(
    *,
    pattern: str,
    replacement: str,
    language: str,
    file_path: str,
    path: str,
) -> str:
    """Build a stable signature for preview/apply pairing."""
    payload = json.dumps(
        {
            "pattern": pattern,
            "replacement": replacement,
            "language": language,
            "file_path": file_path,
            "path": path,
        },
        sort_keys=True,
        separators=(",", ":"),
    )
    return hashlib.sha1(payload.encode()).hexdigest()[:12]


def _preview_store(state) -> dict[str, float]:
    if not hasattr(state, "_recent_edit_previews") or state._recent_edit_previews is None:
        state._recent_edit_previews = {}
    return state._recent_edit_previews


def remember_preview(state, signature: str) -> None:
    """Record a preview signature for later guarded apply."""
    store = _preview_store(state)
    now = time.monotonic()
    store[signature] = now


def has_fresh_preview(state, signature: str, *, ttl_seconds: float) -> bool:
    """Return whether a preview signature was seen recently enough."""
    store = _preview_store(state)
    now = time.monotonic()
    expired = [key for key, seen_at in store.items() if now - seen_at > ttl_seconds]
    for key in expired:
        store.pop(key, None)
    seen_at = store.get(signature)
    return seen_at is not None and now - seen_at <= ttl_seconds


def execute_pattern_rewrite(
    *,
    state,
    pattern: str,
    replacement: str,
    language: str,
    file_path: str,
    path: str,
    dry_run: bool,
    skip_dirs: set[str] | tuple[str, ...],
    preview_context_lines: int,
    preview_max_diff_lines: int,
) -> dict[str, Any]:
    """Execute or preview an AST-backed structural rewrite."""
    from ast_grep_py import SgRoot

    try:
        target_files = _resolve_target_files(
            codebase_path=state.codebase_path,
            file_path=file_path,
            path=path,
            language=language,
            skip_dirs=skip_dirs,
        )
    except FileNotFoundError:
        return {"error": f"File not found: {file_path}"}
    except NotADirectoryError:
        return {"error": f"Path not found: {path}"}

    if not target_files:
        return {"operation": "pattern_rewrite", "changes": 0, "message": "No matching files found"}

    all_results = []
    total_changes = 0
    for fp in target_files:
        source = fp.read_text(errors="replace")
        root_node = SgRoot(source, language.lower())
        node = root_node.root()
        matches = node.find_all(pattern=pattern)
        if not matches:
            continue

        edits = []
        touched_ranges: list[tuple[int, int]] = []
        touched_lines: set[int] = set()
        for match in matches:
            edits.append(match.replace(replacement))
            match_range = match.range()
            start_line = match_range.start.line + 1
            end_line = max(start_line, match_range.end.line + 1)
            touched_ranges.append((start_line, end_line))
            touched_lines.update(range(start_line, end_line + 1))

        updated = node.commit_edits(edits)
        change_count = len(matches)
        total_changes += change_count
        relative_path = _relativize(fp, state.codebase_path)
        result_entry: dict[str, Any] = {
            "file": relative_path,
            "changes": change_count,
            "touched_lines": sorted(touched_lines),
        }
        changed_symbols = _changed_symbols(
            state.graph_engine, fp, touched_ranges, state.codebase_path
        )
        if changed_symbols:
            result_entry["changed_symbols"] = changed_symbols
        if dry_run:
            result_entry["dry_run"] = True
            result_entry.update(
                _diff_payload(
                    source,
                    updated,
                    relative_path,
                    context_lines=preview_context_lines,
                    max_diff_lines=preview_max_diff_lines,
                )
            )
        else:
            fp.write_text(updated)
            result_entry["applied"] = True
        all_results.append(result_entry)

    if not all_results:
        if file_path:
            return {
                "operation": "pattern_rewrite",
                "file": _relativize(target_files[0], state.codebase_path),
                "changes": 0,
                "dry_run": dry_run,
                "message": "No matching files found",
            }
        return {"operation": "pattern_rewrite", "changes": 0, "message": "No matching files found"}

    if len(all_results) == 1 and file_path:
        result = {
            "operation": "pattern_rewrite",
            "file": all_results[0]["file"],
            "changes": all_results[0]["changes"],
            **{
                key: value
                for key, value in all_results[0].items()
                if key not in {"file", "changes"}
            },
        }
        if dry_run:
            result["hint"] = "Set dry_run=false to apply changes"
        return result

    result = {
        "operation": "pattern_rewrite",
        "total_changes": total_changes,
        "files_modified": len(all_results),
        "dry_run": dry_run,
        "results": all_results,
    }
    if dry_run:
        result["hint"] = "Set dry_run=false to apply changes"
    return result
