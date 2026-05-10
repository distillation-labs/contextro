"""Tests for the contextro CLI entrypoint."""

import contextro_mcp.server as server_module
from contextro_mcp import __version__
from contextro_mcp.artifacts.bootstrap import BEGIN_MARKER, build_bootstrap_block
from contextro_mcp.reports.renderers import render_report


def test_main_help_prints_usage(capsys):
    try:
        server_module.main(["--help"])
    except SystemExit as exc:
        assert exc.code == 0
    else:
        raise AssertionError("main(--help) should exit")

    captured = capsys.readouterr()
    assert "Run the Contextro MCP server locally over stdio or HTTP." in captured.out
    assert "--transport" in captured.out
    assert "focus" in captured.out


def test_main_version_prints_version(capsys, monkeypatch):
    called = {"created": False}

    def _unexpected_create_server():
        called["created"] = True
        raise AssertionError("create_server should not run for --version")

    monkeypatch.setattr(server_module, "create_server", _unexpected_create_server)
    server_module.main(["--version"])

    captured = capsys.readouterr()
    assert captured.out.strip() == __version__
    assert called["created"] is False


def test_main_http_args_override_env(monkeypatch):
    calls = {}

    class _FakeServer:
        def run(self, **kwargs):
            calls["kwargs"] = kwargs

    class _FakeState:
        def __init__(self):
            self.shutdown_called = False

        def shutdown(self):
            self.shutdown_called = True

    fake_state = _FakeState()

    monkeypatch.setattr(server_module, "create_server", lambda: _FakeServer())
    monkeypatch.setattr("contextro_mcp.state.get_state", lambda: fake_state)

    server_module.main(["--transport", "http", "--host", "127.0.0.1", "--port", "8765"])

    assert calls["kwargs"] == {
        "transport": "streamable-http",
        "host": "127.0.0.1",
        "port": 8765,
        "path": "/mcp",
    }
    assert fake_state.shutdown_called is True


def test_main_focus_subcommand_prints_json(capsys, monkeypatch):
    class _FakeState:
        def __init__(self):
            self.shutdown_called = False

        def shutdown(self):
            self.shutdown_called = True

    fake_state = _FakeState()

    monkeypatch.setattr("contextro_mcp.state.get_state", lambda: fake_state)
    monkeypatch.setattr(
        "contextro_mcp.cli.runtime.ensure_indexed_state",
        lambda codebase_path=None: fake_state,
    )
    monkeypatch.setattr(
        "contextro_mcp.reports.product.build_focus_report",
        lambda state, path, include_code=True: {"path": path, "role": "test"},
    )

    server_module.main(["focus", "src/main.py", "--format", "json"])

    captured = capsys.readouterr()
    assert '"path": "src/main.py"' in captured.out
    assert fake_state.shutdown_called is True


def test_main_skill_updates_target(tmp_path, monkeypatch):
    class _FakeState:
        def __init__(self):
            self.shutdown_called = False

        def shutdown(self):
            self.shutdown_called = True

    fake_state = _FakeState()
    target = tmp_path / "CLAUDE.md"

    monkeypatch.setattr("contextro_mcp.state.get_state", lambda: fake_state)

    server_module.main(["skill", "--target", str(target)])

    assert BEGIN_MARKER in target.read_text()
    assert fake_state.shutdown_called is True


def test_main_skill_prints_bootstrap_block(capsys, monkeypatch):
    class _FakeState:
        def __init__(self):
            self.shutdown_called = False

        def shutdown(self):
            self.shutdown_called = True

    fake_state = _FakeState()

    monkeypatch.setattr("contextro_mcp.state.get_state", lambda: fake_state)

    server_module.main(["skill"])

    captured = capsys.readouterr()
    assert captured.out == build_bootstrap_block() + "\n"
    assert fake_state.shutdown_called is True


def test_main_skill_invalid_target_exits_cleanly(tmp_path, monkeypatch, capsys):
    class _FakeState:
        def __init__(self):
            self.shutdown_called = False

        def shutdown(self):
            self.shutdown_called = True

    fake_state = _FakeState()

    monkeypatch.setattr("contextro_mcp.state.get_state", lambda: fake_state)

    try:
        server_module.main(["skill", "--target", str(tmp_path / "README.md")])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("main() should exit for invalid bootstrap targets")

    captured = capsys.readouterr()
    assert "Unsupported bootstrap target" in captured.err
    assert fake_state.shutdown_called is True


def test_main_docs_subcommand_uses_codebase_relative_default(tmp_path, capsys, monkeypatch):
    class _FakeState:
        def __init__(self):
            self.shutdown_called = False
            self.codebase_path = tmp_path

        def shutdown(self):
            self.shutdown_called = True

    fake_state = _FakeState()

    monkeypatch.setattr("contextro_mcp.state.get_state", lambda: fake_state)
    monkeypatch.setattr(
        "contextro_mcp.cli.runtime.ensure_indexed_state",
        lambda codebase_path=None: fake_state,
    )
    monkeypatch.setattr(
        "contextro_mcp.artifacts.docs_bundle.build_docs_sections",
        lambda state: {
            "index.md": "# Demo",
            "workflow.md": "# Workflow",
            "architecture.md": "# Architecture",
            "analysis.md": "# Analysis",
            "audit.md": "# Audit",
            "dead-code.md": "# Dead Code",
            "test-coverage.md": "# Test Coverage",
            "circular-dependencies.md": "# Cycles",
            "llms.txt": "demo",
        },
    )

    server_module.main(["docs", "--format", "json"])

    captured = capsys.readouterr()
    assert str((tmp_path / ".contextro-docs").resolve()) in captured.out
    assert fake_state.shutdown_called is True


def test_main_sidecar_clean_subcommand_prints_json(capsys, monkeypatch):
    class _FakeState:
        def __init__(self):
            self.shutdown_called = False

        def shutdown(self):
            self.shutdown_called = True

    fake_state = _FakeState()

    monkeypatch.setattr("contextro_mcp.state.get_state", lambda: fake_state)
    monkeypatch.setattr(
        "contextro_mcp.cli.runtime.ensure_indexed_state",
        lambda codebase_path=None: fake_state,
    )
    monkeypatch.setattr(
        "contextro_mcp.artifacts.sidecars.clean_sidecars",
        lambda state, target_path=None: {"count": 1, "removed": [target_path]},
    )

    server_module.main(["sidecar", "clean", "src/main.py", "--format", "json"])

    captured = capsys.readouterr()
    assert '"count": 1' in captured.out
    assert "src/main.py" in captured.out
    assert fake_state.shutdown_called is True


def test_main_graph_init_subcommand_prints_json(capsys, monkeypatch):
    class _FakeState:
        def __init__(self):
            self.shutdown_called = False

        def shutdown(self):
            self.shutdown_called = True

    fake_state = _FakeState()

    monkeypatch.setattr("contextro_mcp.state.get_state", lambda: fake_state)
    monkeypatch.setattr(
        "contextro_mcp.cli.runtime.ensure_indexed_state",
        lambda codebase_path=None: fake_state,
    )
    monkeypatch.setattr(
        "contextro_mcp.artifacts.graph_workflow.initialize_graph_workflow",
        lambda state, **kwargs: {"workflow": "graph", "mode": "initialized", **kwargs},
    )

    server_module.main(["graph", "init", "src", "--format", "json"])

    captured = capsys.readouterr()
    assert '"workflow": "graph"' in captured.out
    assert '"mode": "initialized"' in captured.out
    assert fake_state.shutdown_called is True


def test_main_graph_watch_subcommand_runs_async_workflow(monkeypatch):
    class _FakeState:
        def __init__(self):
            self.shutdown_called = False

        def shutdown(self):
            self.shutdown_called = True

    fake_state = _FakeState()
    called = {}

    async def _fake_watch_graph_workflow(**kwargs):
        called["kwargs"] = kwargs

    monkeypatch.setattr("contextro_mcp.state.get_state", lambda: fake_state)
    monkeypatch.setattr(
        "contextro_mcp.artifacts.graph_workflow.watch_graph_workflow",
        _fake_watch_graph_workflow,
    )

    server_module.main(
        [
            "graph",
            "watch",
            "src",
            "--include-code",
            "--no-docs",
            "--bootstrap-target",
            "claude",
            "--debounce-seconds",
            "1.5",
            "--format",
            "json",
        ]
    )

    assert called["kwargs"] == {
        "codebase_path": None,
        "target_path": "src",
        "include_code": True,
        "docs_output_dir": "",
        "bootstrap_target": "claude",
        "include_docs": False,
        "debounce_seconds": 1.5,
        "output_format": "json",
    }
    assert fake_state.shutdown_called is True


def test_main_invalid_focus_path_exits_cleanly(monkeypatch, capsys):
    class _FakeState:
        def __init__(self):
            self.shutdown_called = False

        def shutdown(self):
            self.shutdown_called = True

    fake_state = _FakeState()

    monkeypatch.setattr("contextro_mcp.state.get_state", lambda: fake_state)
    monkeypatch.setattr(
        "contextro_mcp.cli.runtime.ensure_indexed_state",
        lambda codebase_path=None: fake_state,
    )

    def _raise_value_error(state, path, include_code=True):
        raise ValueError("bad path")

    monkeypatch.setattr("contextro_mcp.reports.product.build_focus_report", _raise_value_error)

    try:
        server_module.main(["focus", "src/missing.py"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("main() should exit for invalid focus paths")

    captured = capsys.readouterr()
    assert "bad path" in captured.err
    assert fake_state.shutdown_called is True


def test_render_report_normalizes_sets_and_empty_values():
    rendered = render_report({"paths": {"b", "a"}, "empty": []}, "human")
    assert "paths:" in rendered
    assert "- a" in rendered
    assert "- b" in rendered
    assert "empty:" in rendered
    assert "[]" in rendered
