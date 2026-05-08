"""Shared output renderers for CLI and report generation."""

from __future__ import annotations

import json
from dataclasses import asdict, is_dataclass
from enum import Enum
from pathlib import Path
from typing import Any


def render_report(data: Any, output_format: str = "human") -> str:
    """Render nested report data into a stable human/json/markdown form."""
    normalized = _normalize(data)
    if output_format == "json":
        return json.dumps(normalized, indent=2, sort_keys=True)
    if output_format == "markdown":
        return _render_markdown(normalized).strip()
    if output_format == "compact":
        return _render_compact(normalized).strip()
    return _render_human(normalized).strip()


def _normalize(data: Any) -> Any:
    if isinstance(data, Enum):
        return data.value
    if isinstance(data, Path):
        return str(data)
    if is_dataclass(data):
        return _normalize(asdict(data))
    if isinstance(data, dict):
        return {key: _normalize(value) for key, value in data.items()}
    if isinstance(data, (set, frozenset)):
        return [_normalize(item) for item in sorted(data, key=_sort_key)]
    if isinstance(data, tuple):
        return [_normalize(item) for item in data]
    if isinstance(data, list):
        return [_normalize(item) for item in data]
    return data


def _render_human(data: Any, indent: int = 0) -> str:
    prefix = "  " * indent
    if isinstance(data, dict):
        if not data:
            return f"{prefix}{{}}"
        lines: list[str] = []
        for key, value in data.items():
            label = key.replace("_", " ")
            if isinstance(value, (dict, list)):
                lines.append(f"{prefix}{label}:")
                lines.append(_render_human(value, indent + 1))
            else:
                lines.append(f"{prefix}{label}: {value}")
        return "\n".join(line for line in lines if line)
    if isinstance(data, list):
        if not data:
            return f"{prefix}[]"
        lines = []
        for item in data:
            if isinstance(item, (dict, list)):
                lines.append(f"{prefix}-")
                lines.append(_render_human(item, indent + 1))
            else:
                lines.append(f"{prefix}- {item}")
        return "\n".join(lines)
    return f"{prefix}{data}"


def _render_markdown(data: Any, depth: int = 2) -> str:
    if isinstance(data, dict):
        if not data:
            return "{}"
        lines: list[str] = []
        for key, value in data.items():
            heading = "#" * min(depth, 6)
            label = key.replace("_", " ").title()
            if isinstance(value, dict):
                lines.append(f"{heading} {label}")
                lines.append(_render_markdown(value, min(depth + 1, 6)))
            elif isinstance(value, list):
                lines.append(f"{heading} {label}")
                lines.append(_render_markdown(value, min(depth + 1, 6)))
            else:
                lines.append(f"- **{label}:** {value}")
        return "\n\n".join(line for line in lines if line)
    if isinstance(data, list):
        if not data:
            return "[]"
        lines = []
        for item in data:
            if isinstance(item, dict):
                if all(not isinstance(value, (dict, list)) for value in item.values()):
                    compact = ", ".join(f"**{k.replace('_', ' ')}:** {v}" for k, v in item.items())
                    lines.append(f"- {compact}")
                else:
                    lines.append("-")
                    for key, value in item.items():
                        label = key.replace("_", " ").title()
                        if isinstance(value, (dict, list)):
                            lines.append(f"  - **{label}:**")
                            lines.append(_indent_block(_render_markdown(value, depth + 1), "    "))
                        else:
                            lines.append(f"  - **{label}:** {value}")
            elif isinstance(item, list):
                lines.append(f"- {_render_markdown(item, depth + 1)}")
            else:
                lines.append(f"- {item}")
        return "\n".join(lines)
    return str(data)


def _render_compact(data: Any) -> str:
    if isinstance(data, dict):
        if not data:
            return "{}"
        parts = []
        for key, value in data.items():
            if isinstance(value, dict):
                parts.append(f"{key}={{ {_render_compact(value)} }}")
            elif isinstance(value, list):
                rendered = ", ".join(_render_compact(item) for item in value[:10])
                if len(value) > 10:
                    rendered += ", ..."
                parts.append(f"{key}=[{rendered}]")
            else:
                parts.append(f"{key}={value}")
        return " ".join(parts)
    if isinstance(data, list):
        if not data:
            return "[]"
        return ", ".join(_render_compact(item) for item in data)
    return str(data)


def _indent_block(value: str, prefix: str) -> str:
    return "\n".join(f"{prefix}{line}" if line else prefix.rstrip() for line in value.splitlines())


def _sort_key(value: Any) -> str:
    return json.dumps(_normalize(value), sort_keys=True, default=str)
