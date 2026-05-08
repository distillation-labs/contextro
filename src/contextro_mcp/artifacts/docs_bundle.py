"""Packaged docs bundle generation."""

from __future__ import annotations

from pathlib import Path

from contextro_mcp.reports.product import DOCS_SECTION_ORDER, build_docs_sections

DOCS_BUNDLE_SCHEMA_VERSION = 1


def resolve_docs_output_dir(state, output_dir: str | Path | None = None) -> Path:
    """Resolve docs output paths relative to the indexed codebase root."""
    root = Path(state.codebase_path).resolve()
    if output_dir in (None, ""):
        return (root / ".contextro-docs").resolve()

    destination = Path(output_dir).expanduser()
    if not destination.is_absolute():
        destination = root / destination
    return destination.resolve()


def write_docs_bundle(state, output_dir: str | Path | None = None) -> dict[str, object]:
    """Write the packaged docs bundle to the requested directory."""
    output_dir = resolve_docs_output_dir(state, output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    sections = build_docs_sections(state)
    missing = [filename for filename in DOCS_SECTION_ORDER if filename not in sections]
    if missing:
        raise ValueError(
            "Docs bundle missing required sections: " + ", ".join(sorted(missing))
        )

    ordered_filenames = [
        *DOCS_SECTION_ORDER,
        *sorted(filename for filename in sections if filename not in DOCS_SECTION_ORDER),
    ]
    written = []
    documents = []
    for filename in ordered_filenames:
        content = sections[filename]
        path = output_dir / filename
        path.write_text(content.strip() + "\n", encoding="utf-8")
        written.append(str(path))
        documents.append(
            {
                "filename": filename,
                "path": str(path),
                "content_type": "text/plain" if path.suffix == ".txt" else "text/markdown",
            }
        )
    return {
        "bundle_type": "docs_bundle",
        "schema_version": DOCS_BUNDLE_SCHEMA_VERSION,
        "output_dir": str(output_dir),
        "entrypoints": {
            "index": str(output_dir / "index.md"),
            "llms": str(output_dir / "llms.txt"),
        },
        "documents": documents,
        "files": written,
    }
