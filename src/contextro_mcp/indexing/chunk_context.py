"""Helpers for building contextual retrieval text for code chunks."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import PurePosixPath
from typing import Any

from contextro_mcp.core.models import Symbol

_SOURCE_ROOT_HINTS = {"src", "app", "lib", "pkg", "packages", "tests", "test", "cmd"}


@dataclass(frozen=True, slots=True)
class ChunkContextSettings:
    """Normalized settings for chunk-context generation."""

    mode: str = "rich"
    path_depth: int = 4

    @classmethod
    def from_settings(cls, settings: Any) -> "ChunkContextSettings":
        mode = str(getattr(settings, "chunk_context_mode", "rich") or "rich").lower()
        if mode not in {"minimal", "rich"}:
            mode = "rich"

        try:
            path_depth = int(getattr(settings, "chunk_context_path_depth", 4))
        except (TypeError, ValueError):
            path_depth = 4

        return cls(mode=mode, path_depth=max(1, path_depth))


def _normalized_path_parts(filepath: str) -> list[str]:
    normalized = filepath.replace("\\", "/")
    parts = [part for part in PurePosixPath(normalized).parts if part not in {"", "/"}]
    if parts and parts[0].endswith(":"):
        parts = parts[1:]

    # Strip user-home prefixes so chunk hints stay stable across machines while
    # keeping repo-relative path segments intact.
    if len(parts) >= 3 and parts[0] in {"Users", "home"}:
        parts = parts[2:]
    elif len(parts) >= 2 and parts[0] == "root":
        parts = parts[1:]

    return parts


def _module_path_parts(filepath: str, depth: int) -> list[str]:
    parts = _normalized_path_parts(filepath)
    if not parts:
        return []

    max_depth = max(1, depth)
    for index, part in enumerate(parts[:-1]):
        if part in _SOURCE_ROOT_HINTS:
            return parts[index : index + max_depth]

    return parts[-max_depth:]


def normalize_chunk_path(filepath: str, depth: int = 4) -> str:
    """Return a stable, tail-trimmed path hint for chunk text."""
    parts = _normalized_path_parts(filepath)
    if not parts:
        return filepath
    return "/".join(parts[-max(1, depth) :])


def module_hint_from_path(filepath: str, depth: int = 4) -> str:
    """Return a module-like hint derived from the normalized path."""
    tail = _module_path_parts(filepath, depth)
    if not tail:
        return ""

    if tail:
        last = PurePosixPath(tail[-1])
        tail[-1] = last.stem or tail[-1]
    return ".".join(part for part in tail if part)


def build_symbol_context_header(
    symbol: Symbol,
    settings: ChunkContextSettings,
) -> list[str]:
    """Build a compact contextual header for a symbol chunk."""
    file_hint = normalize_chunk_path(symbol.filepath, settings.path_depth)
    qualified = symbol.qualified_name
    symbol_line = f"{symbol.type.value}: {qualified}"

    if settings.mode == "minimal":
        return [f"# {symbol_line} in {file_hint}"]

    module_hint = module_hint_from_path(symbol.filepath, settings.path_depth)
    span = (
        f"L{symbol.line_start}"
        if symbol.line_end <= symbol.line_start
        else f"L{symbol.line_start}-L{symbol.line_end}"
    )

    header = [
        f"# file: {file_hint}",
        f"# symbol: {symbol_line}",
    ]
    if module_hint:
        header.append(f"# module: {module_hint}")
    header.append(f"# span: {span}")
    return header
