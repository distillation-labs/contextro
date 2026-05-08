"""Optional file-adjacent sidecar export mode."""

from __future__ import annotations

from pathlib import Path

from contextro_mcp.artifacts.docs_bundle import write_docs_bundle  # noqa: F401
from contextro_mcp.reports.product import build_focus_report, get_repository_map_for_state
from contextro_mcp.reports.renderers import render_report

SIDECAR_MARKER = "Contextro sidecar v1"

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
    written = []
    for rel_path in files:
        source_path = root / rel_path
        report = build_focus_report(state, rel_path, include_code=include_code)
        sidecar_path = sidecar_path_for(source_path)
        sidecar_path.write_text(_render_sidecar(report, source_path), encoding="utf-8")
        written.append(str(sidecar_path))
    return {"count": len(written), "sidecars": written}


def clean_sidecars(state, *, target_path: str | None = None) -> dict[str, object]:
    """Remove Contextro-managed sidecars."""
    root = Path(state.codebase_path).resolve()
    repo_map = get_repository_map_for_state(state)
    source_files = _select_target_files(root, repo_map, target_path)
    removed = []
    for rel_path in source_files:
        candidate = sidecar_path_for(root / rel_path)
        if candidate.exists() and _is_contextro_sidecar(candidate):
            candidate.unlink()
            removed.append(str(candidate))
    return {"count": len(removed), "removed": removed}


def sidecar_path_for(source_path: Path) -> Path:
    """Return the file-adjacent sidecar path for a source file."""
    return source_path.with_name(f"{source_path.stem}.graph{source_path.suffix}")


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
    rendered = render_report(report, "markdown")
    lines = [f"{prefix} {SIDECAR_MARKER}", f"{prefix} source: {source_path.name}"]
    for line in rendered.splitlines():
        if line.strip():
            lines.append(f"{prefix} {line}")
        else:
            lines.append(prefix)
    return "\n".join(lines) + "\n"


def _is_contextro_sidecar(path: Path) -> bool:
    try:
        first_lines = "\n".join(path.read_text(encoding="utf-8").splitlines()[:3])
    except UnicodeDecodeError:
        first_lines = "\n".join(path.read_text(encoding="utf-8", errors="replace").splitlines()[:3])
    return SIDECAR_MARKER in first_lines


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
