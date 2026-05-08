"""Agent bootstrap block generation and idempotent file updates."""

from __future__ import annotations

import re
from pathlib import Path

BEGIN_MARKER = "<!-- BEGIN CONTEXTRO BOOTSTRAP -->"
END_MARKER = "<!-- END CONTEXTRO BOOTSTRAP -->"
SUPPORTED_BOOTSTRAP_FILENAMES = ("CLAUDE.md", "AGENTS.md", ".cursorrules")
_TARGET_ALIASES = {
    "claude": "CLAUDE.md",
    "claude.md": "CLAUDE.md",
    "agents": "AGENTS.md",
    "agents.md": "AGENTS.md",
    "cursor": ".cursorrules",
    "cursorrules": ".cursorrules",
    ".cursorrules": ".cursorrules",
}
_BLOCK_PATTERN = re.compile(
    rf"{re.escape(BEGIN_MARKER)}.*?{re.escape(END_MARKER)}",
    re.DOTALL,
)


def build_bootstrap_block() -> str:
    """Return the managed instruction block for coding agents."""
    body = "\n".join(
        [
            BEGIN_MARKER,
            "Use Contextro before browsing files manually.",
            "",
            "1. Start with `contextro restore` or the MCP `restore()` tool for repo re-entry.",
            (
                "2. Use `contextro focus path/to/file` or the MCP `focus(path='...')` "
                "tool before editing a file."
            ),
            (
                "3. Prefer Contextro tools over raw file reads: `search`, "
                "`find_symbol`, `impact`, `focus`, `audit`."
            ),
            "4. If sidecars exist (`*.graph.*`), read them before opening the source file.",
            (
                "5. Use `contextro audit` or the MCP `audit()` tool for complexity, "
                "dead code, cycles, and static coverage gaps."
            ),
            (
                "6. Use `contextro restore` / `session_snapshot()` when resuming "
                "after compaction or a long pause."
            ),
            "7. Retrieve full code only when the focused or sidecar summary is not enough.",
            END_MARKER,
        ]
    )
    return body


def resolve_bootstrap_target(target_path: Path) -> Path:
    """Resolve supported bootstrap targets and common aliases."""
    if target_path.exists() and target_path.is_dir():
        raise ValueError(
            "Bootstrap target must be a file named CLAUDE.md, AGENTS.md, or .cursorrules."
        )

    canonical_name = _TARGET_ALIASES.get(target_path.name.lower())
    if canonical_name is None:
        supported = ", ".join(SUPPORTED_BOOTSTRAP_FILENAMES)
        raise ValueError(
            f"Unsupported bootstrap target '{target_path.name}'. Use {supported}, "
            "or the claude/agents/cursor aliases."
        )

    return target_path.with_name(canonical_name)


def _merge_bootstrap(existing: str, block: str) -> tuple[str, bool]:
    """Insert or replace the managed block without duplicating it."""
    has_begin = BEGIN_MARKER in existing
    has_end = END_MARKER in existing
    if has_begin != has_end:
        raise ValueError(
            "Found an incomplete Contextro bootstrap block. Remove the partial managed block "
            "and retry."
        )

    if not has_begin:
        prefix = existing.rstrip("\r\n")
        suffix = ""
    else:
        matches = list(_BLOCK_PATTERN.finditer(existing))
        if not matches:
            raise ValueError(
                "Found malformed Contextro bootstrap markers. Remove the broken managed block "
                "and retry."
            )

        prefix = existing[: matches[0].start()].rstrip("\r\n")
        suffix_parts: list[str] = []
        cursor = matches[0].end()
        for match in matches[1:]:
            suffix_parts.append(existing[cursor : match.start()])
            cursor = match.end()
        suffix_parts.append(existing[cursor:])
        suffix = "".join(suffix_parts).lstrip("\r\n")

    updated = "\n\n".join(part for part in (prefix, block, suffix) if part)
    if updated and not updated.endswith("\n"):
        updated += "\n"
    return updated, updated != existing


def write_bootstrap(target_path: Path) -> dict[str, str | bool]:
    """Create or update a target file with the managed Contextro block."""
    target_path = resolve_bootstrap_target(target_path).resolve()
    block = build_bootstrap_block()
    existing = target_path.read_text(encoding="utf-8") if target_path.exists() else ""
    updated, changed = _merge_bootstrap(existing, block)

    target_path.parent.mkdir(parents=True, exist_ok=True)
    if changed:
        target_path.write_text(updated, encoding="utf-8")
    return {"path": str(target_path), "changed": changed}
