"""Tests for productized Contextro tools and artifact surfaces."""

import asyncio
import json
from pathlib import Path

from contextro_mcp.analysis.repository_map import build_repository_map
from contextro_mcp.analysis.static_analysis import (
    analyze_circular_dependencies,
    analyze_dead_code,
    analyze_test_coverage_map,
)
from contextro_mcp.engines.graph_engine import RustworkxCodeGraph
from contextro_mcp.state import get_state
from tests.conftest import _call_tool, _setup_indexed


def _create_product_codebase(root: Path) -> Path:
    src = root / "src"
    tests = root / "tests"
    src.mkdir()
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


def _create_pyproject_reexport_codebase(root: Path) -> Path:
    app = root / "app"
    tests = root / "tests"
    app.mkdir()
    tests.mkdir()

    (root / "pyproject.toml").write_text(
        "[project]\n"
        "name = 'demo-app'\n"
        "version = '0.0.1'\n\n"
        "[project.scripts]\n"
        "demo = 'app.cli:main'\n"
    )
    (app / "__init__.py").write_text("from .impl import helper\n")
    (app / "impl.py").write_text("def helper():\n    return 42\n")
    (app / "cli.py").write_text("from app import helper\n\n\ndef main():\n    return helper()\n")
    (tests / "test_cli.py").write_text(
        "from app.cli import main\n\n\n"
        "def test_main():\n"
        "    assert main() == 42\n"
    )
    return root


def _create_package_script_codebase(root: Path) -> Path:
    scripts = root / "scripts"
    src = root / "src"
    scripts.mkdir()
    src.mkdir()

    (root / "package.json").write_text(
        json.dumps(
            {
                "scripts": {
                    "start": "node ./scripts/dev.ts",
                    "test": "vitest run",
                }
            }
        )
    )
    (scripts / "dev.ts").write_text("import { boot } from '../src/bootstrap'\nboot()\n")
    (src / "bootstrap.ts").write_text("export function boot() {\n  return 1\n}\n")
    return root


def _create_test_only_private_symbol_codebase(root: Path) -> Path:
    src = root / "src"
    tests = root / "tests"
    src.mkdir()
    tests.mkdir()

    (src / "__init__.py").write_text("")
    (src / "main.py").write_text(
        "from src.helpers import public\n\n\n"
        "def run():\n"
        "    return public()\n"
    )
    (src / "helpers.py").write_text(
        "def public():\n"
        "    return 1\n\n\n"
        "def _test_only():\n"
        "    return 2\n"
    )
    (tests / "test_helpers.py").write_text(
        "from src.helpers import _test_only\n\n\n"
        "def test_private():\n"
        "    assert _test_only() == 2\n"
    )
    return root


def _create_self_cycle_codebase(root: Path) -> Path:
    src = root / "src"
    src.mkdir()
    (src / "__init__.py").write_text("")
    (src / "loop.py").write_text(
        "from src.loop import main\n\n\n"
        "def main():\n"
        "    return 1\n"
    )
    return root


class TestProductTools:
    def test_focus_restore_audit_and_static_tools(self, tmp_path):
        codebase = _create_product_codebase(tmp_path)

        async def run():
            mcp, _, _ = await _setup_indexed(codebase, tmp_path / ".contextro")
            focus = await _call_tool(
                mcp,
                "focus",
                {"path": "src/main.py", "include_code": False},
            )
            restore = await _call_tool(mcp, "restore")
            audit = await _call_tool(mcp, "audit")
            dead_code = await _call_tool(mcp, "dead_code")
            circular = await _call_tool(mcp, "circular_dependencies")
            coverage = await _call_tool(mcp, "test_coverage_map")
            return focus, restore, audit, dead_code, circular, coverage

        focus, restore, audit, dead_code, circular, coverage = asyncio.run(run())

        assert focus["path"] == "src/main.py"
        assert "src/utils.py" in focus["imports"]
        assert restore["entry_points"]
        assert "summary" in audit
        assert audit["summary"]["dead_files"] >= 1
        assert "src/dead.py" in dead_code["unused_files"]
        assert circular["summary"]["cycle_count"] >= 1
        cycle_files = {item for cycle in circular["cycles"] for item in cycle["files"]}
        assert {"src/cycle_a.py", "src/cycle_b.py"} <= cycle_files
        assert "src/dead.py" in coverage["uncovered_files"]

    def test_artifact_tools(self, tmp_path):
        codebase = _create_product_codebase(tmp_path)
        docs_dir = tmp_path / "bundle"
        bootstrap_target = tmp_path / "CLAUDE.md"

        async def run():
            mcp, _, _ = await _setup_indexed(codebase, tmp_path / ".contextro")
            sidecars = await _call_tool(
                mcp,
                "sidecar_export",
                {"path": "src/main.py", "include_code": False},
            )
            prompt = await _call_tool(mcp, "skill_prompt")
            wrote_prompt = await _call_tool(
                mcp,
                "skill_prompt",
                {"target_path": str(bootstrap_target)},
            )
            docs = await _call_tool(mcp, "docs_bundle", {"output_dir": "bundle"})
            return mcp, sidecars, prompt, wrote_prompt, docs

        mcp, sidecars, prompt, wrote_prompt, docs = asyncio.run(run())

        sidecar_path = tmp_path / "src" / "main.graph.py"
        assert sidecars["count"] == 1
        assert sidecar_path.exists()
        assert "Contextro sidecar v1" in sidecar_path.read_text()
        assert "<!-- BEGIN CONTEXTRO BOOTSTRAP -->" in prompt["content"]
        assert wrote_prompt["path"] == str(bootstrap_target.resolve())
        assert bootstrap_target.exists()
        assert docs["output_dir"] == str(docs_dir.resolve())
        assert (docs_dir / "index.md").exists()
        assert (docs_dir / "architecture.md").exists()
        assert (docs_dir / "audit.md").exists()
        assert (docs_dir / "llms.txt").exists()
        cleaned = asyncio.run(
            _call_tool(mcp, "sidecar_export", {"path": "src/main.py", "clean": True})
        )
        assert cleaned["count"] == 1
        assert not sidecar_path.exists()

    def test_overview_and_recall_use_disclosure_when_threshold_is_low(self, tmp_path, monkeypatch):
        codebase = _create_product_codebase(tmp_path)
        long_memory = "Architecture note for disclosure testing. " * 40
        monkeypatch.setenv("CTX_SEARCH_SANDBOX_THRESHOLD_TOKENS", "1")

        async def run():
            mcp, _, _ = await _setup_indexed(codebase, tmp_path / ".contextro")
            overview = await _call_tool(mcp, "overview")
            await _call_tool(
                mcp,
                "remember",
                {"content": long_memory, "memory_type": "note", "project": "demo"},
            )
            recall = await _call_tool(mcp, "recall", {"query": "disclosure testing"})
            return overview, recall

        overview, recall = asyncio.run(run())

        assert "sandbox_ref" in overview
        assert overview["sandbox_ref"].startswith("sx_")
        assert overview["total_files"] >= 1

        assert "sandbox_ref" in recall
        assert recall["sandbox_ref"].startswith("sx_")
        assert recall["total"] == 1
        assert len(recall["memories"]) == 1
        assert recall["memories"][0]["content"].endswith("…")
        assert len(recall["memories"][0]["content"]) < len(long_memory)

    def test_sidecar_export_rejects_paths_outside_indexed_codebase(self, tmp_path):
        codebase = _create_product_codebase(tmp_path)

        async def run():
            mcp, _, _ = await _setup_indexed(codebase, tmp_path / ".contextro")
            return await _call_tool(mcp, "sidecar_export", {"path": str(tmp_path.parent)})

        result = asyncio.run(run())

        assert "error" in result
        assert "outside indexed codebase" in result["error"]

    def test_repository_map_detects_package_json_script_entry_points(self, tmp_path):
        codebase = _create_package_script_codebase(tmp_path)

        repo_map = build_repository_map(codebase, RustworkxCodeGraph())

        assert "scripts/dev.ts" in repo_map.entry_points

    def test_repository_map_resolves_python_reexports_for_coverage(self, tmp_path):
        codebase = _create_pyproject_reexport_codebase(tmp_path)

        repo_map = build_repository_map(codebase, RustworkxCodeGraph())
        coverage = analyze_test_coverage_map(repo_map, RustworkxCodeGraph())
        coverage_by_path = {item["path"]: item for item in coverage["file_coverage"]}

        assert "app/cli.py" in repo_map.entry_points
        assert not any(link.specifier.endswith("helper") for link in repo_map.unresolved_imports)
        assert coverage_by_path["app/cli.py"]["tests"] == ["tests/test_cli.py"]
        assert coverage_by_path["app/impl.py"]["tests"] == ["tests/test_cli.py"]
        assert "app/impl.py" not in coverage["uncovered_files"]

    def test_dead_code_ignores_test_only_callers(self, tmp_path):
        codebase = _create_test_only_private_symbol_codebase(tmp_path)

        async def run():
            await _setup_indexed(codebase, tmp_path / ".contextro")
            state = get_state()
            repo_map = build_repository_map(codebase, state.graph_engine)
            return analyze_dead_code(repo_map, state.graph_engine)

        dead_code = asyncio.run(run())

        assert "src/helpers.py" not in dead_code["unused_files"]
        assert any(item["name"] == "_test_only" for item in dead_code["unused_symbols"])

    def test_circular_dependencies_reports_self_cycles(self, tmp_path):
        codebase = _create_self_cycle_codebase(tmp_path)

        repo_map = build_repository_map(codebase, RustworkxCodeGraph())
        circular = analyze_circular_dependencies(repo_map)

        assert circular["summary"]["cycle_count"] == 1
        assert circular["summary"]["self_cycles"] == 1
        assert circular["cycles"][0]["files"] == ["src/loop.py"]
        assert circular["cycles"][0]["kind"] == "self_cycle"
        assert circular["cycles"][0]["cycle"] == ["src/loop.py", "src/loop.py"]
