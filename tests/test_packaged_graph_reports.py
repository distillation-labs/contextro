"""Narrow tests for packaged audit reports and local docs bundles."""

import asyncio
from pathlib import Path

import pytest

from contextro_mcp.artifacts.docs_bundle import DOCS_BUNDLE_SCHEMA_VERSION, write_docs_bundle
from contextro_mcp.reports.product import (
    AUDIT_SCHEMA_VERSION,
    DOCS_SECTION_ORDER,
    build_audit_report,
    build_docs_sections,
)
from contextro_mcp.state import get_state
from tests.conftest import _setup_indexed


def _create_product_codebase(root: Path) -> Path:
    src = root / "src"
    tests = root / "tests"
    src.mkdir(parents=True)
    tests.mkdir()

    (src / "__init__.py").write_text("")
    (src / "main.py").write_text(
        "from src.utils import helper\n\n\n"
        "def hello():\n"
        "    helper()\n"
    )
    (src / "utils.py").write_text(
        'def helper():\n'
        '    """Utility entry."""\n'
        "    return 42\n\n\n"
        "def _unused_helper():\n"
        "    return 0\n"
    )
    (src / "dead.py").write_text("def dead_func():\n    return 1\n")
    (src / "cycle_a.py").write_text("from src import cycle_b\n")
    (src / "cycle_b.py").write_text("from src import cycle_a\n")
    (tests / "test_main.py").write_text(
        "from src.main import hello\n\n\n"
        "def test_hello():\n"
        "    hello()\n"
    )
    return root


def _setup_state(codebase: Path, index_dir: Path):
    async def run():
        await _setup_indexed(codebase, index_dir)
        return get_state()

    return asyncio.run(run())


def test_build_audit_report_has_stable_schema_and_structured_recommendations(tmp_path):
    state = _setup_state(
        _create_product_codebase(tmp_path / "codebase"),
        tmp_path / ".contextro",
    )

    audit = build_audit_report(state)

    assert audit["report_type"] == "audit"
    assert audit["schema_version"] == AUDIT_SCHEMA_VERSION
    assert audit["blast_radius_hotspots"] == audit["hub_risks"]
    assert "coverage_ratio" in audit["summary"]
    assert "uncovered_hub_files" in audit["summary"]
    assert audit["recommendations"] == [
        item["action"] for item in audit["recommendation_details"]
    ]
    assert any(
        item["category"] == "cycles" and item["priority"] == "high"
        for item in audit["recommendation_details"]
    )
    assert any(item["category"] == "coverage" for item in audit["recommendation_details"])


def test_build_docs_sections_include_navigation_and_first_class_audit_content(tmp_path):
    state = _setup_state(
        _create_product_codebase(tmp_path / "codebase"),
        tmp_path / ".contextro",
    )

    sections = build_docs_sections(state)

    assert tuple(sections.keys()) == DOCS_SECTION_ORDER
    assert "# Contextro Docs Bundle" in sections["index.md"]
    assert "[Architecture](architecture.md)" in sections["index.md"]
    assert "[Workflow](workflow.md)" in sections["index.md"]
    assert "[Audit](audit.md)" in sections["index.md"]
    assert "[Dead Code](dead-code.md)" in sections["index.md"]
    assert "[Test Coverage](test-coverage.md)" in sections["index.md"]
    assert "[Circular Dependencies](circular-dependencies.md)" in sections["index.md"]
    assert "## Audit Snapshot" in sections["index.md"]
    assert "## Layer Breakdown" in sections["architecture.md"]
    assert "## Hub Files" in sections["architecture.md"]
    assert "contextro graph watch" in sections["workflow.md"]
    assert "[analysis]" in sections["workflow.md"]
    assert "## Prioritized Recommendations" in sections["audit.md"]
    assert "## Blast Radius Hotspots" in sections["audit.md"]
    assert "## Unused Files" in sections["dead-code.md"]
    assert "## Covered File Samples" in sections["test-coverage.md"]
    assert "## Cycles" in sections["circular-dependencies.md"]
    assert "Read order:" in sections["llms.txt"]
    assert "index.md - bundle overview" in sections["llms.txt"]
    assert "workflow.md - how to use `.graph.*` sidecars" in sections["llms.txt"]


def test_write_docs_bundle_returns_stable_manifest(tmp_path):
    codebase = _create_product_codebase(tmp_path / "codebase")
    state = _setup_state(codebase, tmp_path / ".contextro")

    bundle = write_docs_bundle(state, "bundle")

    assert bundle["bundle_type"] == "docs_bundle"
    assert bundle["schema_version"] == DOCS_BUNDLE_SCHEMA_VERSION
    assert bundle["output_dir"] == str((codebase / "bundle").resolve())
    assert bundle["entrypoints"]["index"] == str(
        (codebase / "bundle" / "index.md").resolve()
    )
    assert bundle["entrypoints"]["llms"] == str(
        (codebase / "bundle" / "llms.txt").resolve()
    )
    assert [doc["filename"] for doc in bundle["documents"]] == list(DOCS_SECTION_ORDER)
    assert all(
        Path(doc["path"]).read_text(encoding="utf-8").endswith("\n")
        for doc in bundle["documents"]
    )


def test_write_docs_bundle_requires_core_sections(tmp_path, monkeypatch):
    class _State:
        codebase_path = tmp_path

    monkeypatch.setattr(
        "contextro_mcp.artifacts.docs_bundle.build_docs_sections",
        lambda state: {"index.md": "# Demo"},
    )

    with pytest.raises(ValueError, match="missing required sections"):
        write_docs_bundle(_State(), "bundle")
