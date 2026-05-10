"""Optional file-adjacent sidecar export mode."""

from __future__ import annotations

from pathlib import Path
from typing import Iterable

from contextro_mcp.artifacts.docs_bundle import write_docs_bundle  # noqa: F401
from contextro_mcp.reports.product import build_sidecar_report, get_repository_map_for_state

SIDECAR_MARKER = "Contextro sidecar v2"
SIDECAR_MARKER_PREFIX = "Contextro sidecar v"

_COMMENT_PREFIXES = {
    ".py": "#",
    ".rb": "#",
    ".sh": "#",
    ".bash": "#",
    ".js": "//",
    ".jsx": "//",
    ".ts": "//",
    ".tsx": "//",
    ".mjs": "//",
    ".cjs": "//",
    ".go": "//",
    ".rs": "//",
    ".java": "//",
    ".c": "//",
    ".cc": "//",
    ".cpp": "//",
    ".h": "//",
    ".hpp": "//",
}


def export_sidecars(
    state,
    *,
    target_path: str | None = None,
    include_code: bool = False,
) -> dict[str, object]:
    """Export file-adjacent `.graph.*` summaries."""
    root = Path(state.codebase_path).resolve()
    repo_map = get_repository_map_for_state(state)
    files = _select_target_files(root, repo_map, target_path)
    written = write_sidecars_for_paths(state, files, include_code=include_code)
    return {"count": len(written), "sidecars": written}


def clean_sidecars(state, *, target_path: str | None = None) -> dict[str, object]:
    """Remove Contextro-managed sidecars."""
    root = Path(state.codebase_path).resolve()
    removed = []
    for candidate in _iter_sidecar_candidates(root, target_path):
        if candidate.exists() and _is_contextro_sidecar(candidate):
            candidate.unlink()
            removed.append(str(candidate))
    return {"count": len(removed), "removed": removed}


def sidecar_path_for(source_path: Path) -> Path:
    """Return the file-adjacent sidecar path for a source file."""
    return source_path.with_name(f"{source_path.stem}.graph{source_path.suffix}")


def write_sidecars_for_paths(
    state,
    relative_paths: Iterable[str],
    *,
    include_code: bool = False,
) -> list[str]:
    """Write sidecars for an explicit set of repo-relative paths."""
    root = Path(state.codebase_path).resolve()
    written = []
    for rel_path in sorted(dict.fromkeys(relative_paths)):
        source_path = root / rel_path
        report = build_sidecar_report(state, rel_path, include_code=include_code)
        sidecar_path = sidecar_path_for(source_path)
        sidecar_path.write_text(_render_sidecar(report, source_path), encoding="utf-8")
        written.append(str(sidecar_path))
    return written


def _select_target_files(root: Path, repo_map, target_path: str | None) -> list[str]:
    if not target_path:
        return sorted(repo_map.modules)

    candidate, relative_path = _resolve_target_path(root, target_path)

    if candidate.is_file():
        if relative_path not in repo_map.modules:
            raise ValueError(f"File not found in indexed codebase: {target_path}")
        return [relative_path]

    prefix = relative_path.rstrip("/")
    if prefix in {"", "."}:
        return sorted(repo_map.modules)
    prefix += "/"
    matches = sorted(path for path in repo_map.modules if path.startswith(prefix))
    if matches:
        return matches
    raise ValueError(f"No indexed files matched: {target_path}")


def _render_sidecar(report: dict[str, object], source_path: Path) -> str:
    prefix = _COMMENT_PREFIXES.get(source_path.suffix.lower(), "#")
    lines = [
        f"{prefix} {SIDECAR_MARKER}",
        f"{prefix} source: {report['path']}",
        f"{prefix} generated-by: contextro sidecar export",
        prefix,
    ]
    lines.extend(_render_section(prefix, "overview", report.get("overview", {})))
    lines.append(prefix)
    lines.extend(_render_symbols_section(prefix, report.get("symbols", [])))
    lines.append(prefix)
    lines.extend(_render_section(prefix, "deps", report.get("deps", {})))
    lines.append(prefix)
    lines.extend(_render_section(prefix, "calls", report.get("calls", {})))
    lines.append(prefix)
    lines.extend(_render_section(prefix, "impact", report.get("impact", {})))
    lines.append(prefix)
    lines.extend(_render_section(prefix, "analysis", report.get("analysis", {})))
    if report.get("code_preview"):
        lines.append(prefix)
        lines.append(f"{prefix} [code]")
        for line in str(report["code_preview"]).splitlines():
            lines.append(f"{prefix} {line}" if line else prefix)
    return "\n".join(lines) + "\n"


def _is_contextro_sidecar(path: Path) -> bool:
    try:
        first_lines = "\n".join(path.read_text(encoding="utf-8").splitlines()[:3])
    except UnicodeDecodeError:
        first_lines = "\n".join(path.read_text(encoding="utf-8", errors="replace").splitlines()[:3])
    return SIDECAR_MARKER_PREFIX in first_lines


def _resolve_target_path(root: Path, target_path: str) -> tuple[Path, str]:
    candidate = Path(target_path).expanduser()
    if not candidate.is_absolute():
        candidate = (root / candidate).resolve()
    else:
        candidate = candidate.resolve()

    try:
        relative_path = candidate.relative_to(root).as_posix()
    except ValueError as exc:
        raise ValueError(f"Path is outside indexed codebase: {target_path}") from exc

    return candidate, relative_path


def _iter_sidecar_candidates(root: Path, target_path: str | None) -> Iterable[Path]:
    if target_path is None:
        candidates = root.rglob("*.graph.*")
    else:
        candidate, _ = _resolve_target_path(root, target_path)
        if candidate.is_file() or (not candidate.exists() and candidate.suffix):
            candidates = [sidecar_path_for(candidate)]
        else:
            candidates = candidate.rglob("*.graph.*")
    for sidecar in candidates:
        if sidecar.is_file():
            yield sidecar


def _render_symbols_section(prefix: str, symbols: list[dict[str, object]]) -> list[str]:
    lines = [f"{prefix} [symbols]"]
    if not symbols:
        lines.append(f"{prefix} none")
        return lines
    for item in symbols:
        lines.append(
            f"{prefix} {item['type'].lower():<10} {item['name']}  "
            f"line {item['line']}  span {item['line_count']}"
        )
    return lines


def _render_section(prefix: str, section: str, data: dict[str, object]) -> list[str]:
    lines = [f"{prefix} [{section}]"]
    if not data:
        lines.append(f"{prefix} none")
        return lines
    for key, value in data.items():
        label = key.replace("_", "-")
        lines.extend(_render_value(prefix, label, value))
    return lines


def _render_value(prefix: str, label: str, value: object) -> list[str]:
    if isinstance(value, dict):
        lines = [f"{prefix} {label}:"]
        for nested_key, nested_value in value.items():
            lines.extend(_render_value(prefix, f"  {nested_key}".replace("_", "-"), nested_value))
        return lines
    if isinstance(value, list):
        if not value:
            return [f"{prefix} {label:<14} (none)"]
        rendered = []
        for item in value:
            if isinstance(item, dict):
                summary = ", ".join(
                    f"{nested_key.replace('_', '-')}={nested_value}"
                    for nested_key, nested_value in item.items()
                )
                rendered.append(f"{prefix} {label:<14} {summary}")
            else:
                rendered.append(f"{prefix} {label:<14} {item}")
        return rendered
    if isinstance(value, bool):
        return [f"{prefix} {label:<14} {'yes' if value else 'no'}"]
    return [f"{prefix} {label:<14} {value}"]
