"""Productized graph workflow helpers for sidecars, docs, and live refresh."""

from __future__ import annotations

import asyncio
from pathlib import Path
from typing import Iterable

from contextro_mcp.artifacts.bootstrap import write_bootstrap
from contextro_mcp.artifacts.docs_bundle import resolve_docs_output_dir, write_docs_bundle
from contextro_mcp.artifacts.sidecars import (
    clean_sidecars,
    export_sidecars,
    write_sidecars_for_paths,
)
from contextro_mcp.cli.runtime import ensure_indexed_state, reindex_state
from contextro_mcp.parsing.file_watcher import DebouncedFileWatcher
from contextro_mcp.reports.product import get_repository_map_for_state
from contextro_mcp.reports.renderers import render_report
from contextro_mcp.state import get_state

_WORKFLOW_CONFIG_NAMES = {
    "package.json",
    "pyproject.toml",
    "tsconfig.json",
    "tsconfig.base.json",
    "setup.py",
    "setup.cfg",
}
_BOOTSTRAP_FILENAMES = {"CLAUDE.md", "AGENTS.md", ".cursorrules"}
_IGNORED_DIRS = {".git", ".contextro", ".contextro-docs", "__pycache__"}


def initialize_graph_workflow(
    state,
    *,
    target_path: str | None = ".",
    include_code: bool = False,
    docs_output_dir: str | None = None,
    bootstrap_target: str | None = None,
    include_docs: bool = True,
) -> dict[str, object]:
    """Initialize the local-first graph workflow for a repository."""
    root = Path(state.codebase_path).resolve()
    sidecars = export_sidecars(state, target_path=target_path, include_code=include_code)
    docs_bundle: dict[str, object] = {"skipped": True}
    if include_docs:
        docs_bundle = write_docs_bundle(state, docs_output_dir)

    bootstrap: dict[str, object] = {"skipped": True}
    if bootstrap_target:
        bootstrap = write_bootstrap(Path(root / bootstrap_target))

    return {
        "workflow": "graph",
        "mode": "initialized",
        "codebase": str(root),
        "sidecars": sidecars,
        "docs_bundle": docs_bundle,
        "bootstrap": bootstrap,
        "next_steps": [
            "Run `contextro graph watch` to keep sidecars fresh while editing.",
            "Read `*.graph.*` sidecars before opening the source file.",
            "Open `.contextro-docs/index.md` or `llms.txt` for the repo-wide graph briefing.",
        ],
    }


async def watch_graph_workflow(
    *,
    codebase_path: str | None = None,
    target_path: str | None = ".",
    include_code: bool = False,
    docs_output_dir: str | None = None,
    bootstrap_target: str | None = None,
    include_docs: bool = True,
    debounce_seconds: float = 2.0,
    output_format: str = "human",
) -> None:
    """Run the live graph workflow watcher until interrupted."""
    state = ensure_indexed_state(codebase_path)
    summary = initialize_graph_workflow(
        state,
        target_path=target_path,
        include_code=include_code,
        docs_output_dir=docs_output_dir,
        bootstrap_target=bootstrap_target,
        include_docs=include_docs,
    )
    print(render_report(summary, output_format))
    print("Watching for graph-affecting file changes. Press Ctrl+C to stop.")

    root = Path(state.codebase_path).resolve()
    docs_dir = resolve_docs_output_dir(state, docs_output_dir) if include_docs else None
    bootstrap_path = (
        Path(summary["bootstrap"]["path"]).resolve()
        if isinstance(summary.get("bootstrap"), dict) and summary["bootstrap"].get("path")
        else None
    )
    watcher: DebouncedFileWatcher | None = None

    async def _refresh() -> None:
        nonlocal state
        assert watcher is not None
        changed_paths = watcher.consume_recent_changes()
        relative_paths = _relative_paths(changed_paths, root)
        full_refresh = not relative_paths or any(
            _requires_full_refresh(path) for path in relative_paths
        )
        state = reindex_state(str(root), full=full_refresh)
        repo_map = get_repository_map_for_state(state)
        if not full_refresh and any(path not in repo_map.modules for path in relative_paths):
            full_refresh = True

        if full_refresh:
            clean_sidecars(state, target_path=target_path)
            sidecars = export_sidecars(state, target_path=target_path, include_code=include_code)
        else:
            affected = _expand_targets(repo_map, relative_paths)
            sidecar_paths = write_sidecars_for_paths(state, affected, include_code=include_code)
            sidecars = {"count": len(sidecar_paths), "sidecars": sidecar_paths}

        docs_bundle: dict[str, object] = {"skipped": True}
        if include_docs:
            docs_bundle = write_docs_bundle(state, docs_output_dir)

        changed = ", ".join(relative_paths[:6]) if relative_paths else "full refresh"
        docs_output = ""
        if include_docs:
            docs_output = str(docs_bundle.get("output_dir", docs_dir or ""))
        print(
            render_report(
                {
                    "event": "graph_refresh",
                    "changed": changed,
                    "sidecars": sidecars["count"],
                    "docs_updated": include_docs,
                    "docs_dir": docs_output,
                },
                "compact" if output_format == "json" else output_format,
            )
        )

    watcher = DebouncedFileWatcher(
        root,
        callback=_refresh,
        debounce_delay=debounce_seconds,
        should_ignore_path=lambda path, project_root: _should_ignore_workflow_path(
            path,
            project_root,
            docs_dir=docs_dir,
            bootstrap_path=bootstrap_path,
        ),
    )
    state.file_watcher_instance = watcher
    await watcher.start()
    try:
        while True:
            await asyncio.sleep(3600)
    finally:
        await watcher.stop()
        if get_state().file_watcher_instance is watcher:
            get_state().file_watcher_instance = None


def _relative_paths(paths: Iterable[Path], root: Path) -> list[str]:
    relative = []
    for path in paths:
        try:
            relative_path = path.resolve(strict=False).relative_to(root).as_posix()
        except ValueError:
            continue
        if relative_path:
            relative.append(relative_path)
    return sorted(dict.fromkeys(relative))


def _expand_targets(repo_map, changed_paths: Iterable[str]) -> list[str]:
    affected: set[str] = set()
    for rel_path in changed_paths:
        module = repo_map.modules.get(rel_path)
        if module is None:
            continue
        affected.add(rel_path)
        affected.update(module.imports)
        affected.update(module.dependents)
        affected.update(module.calls)
        affected.update(module.called_by)
    return sorted(affected)


def _requires_full_refresh(relative_path: str) -> bool:
    path = Path(relative_path)
    return path.name in _WORKFLOW_CONFIG_NAMES or path.suffix == ".graph"


def _should_ignore_workflow_path(
    path: Path,
    project_root: Path,
    *,
    docs_dir: Path | None,
    bootstrap_path: Path | None,
) -> bool:
    try:
        relative = path.resolve(strict=False).relative_to(project_root.resolve())
    except ValueError:
        return True

    if any(part in _IGNORED_DIRS for part in relative.parts):
        return True
    if ".graph." in path.name:
        return True
    if docs_dir is not None:
        try:
            path.resolve(strict=False).relative_to(docs_dir.resolve())
            return True
        except ValueError:
            pass
    if bootstrap_path is not None and path.resolve(strict=False) == bootstrap_path:
        return True
    if path.name in _BOOTSTRAP_FILENAMES:
        return True
    return False
