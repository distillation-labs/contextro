"""Tests for bootstrap block generation and target handling."""

from __future__ import annotations

import asyncio

import pytest

import contextro_mcp.server as server_module
from contextro_mcp.artifacts.bootstrap import (
    BEGIN_MARKER,
    END_MARKER,
    build_bootstrap_block,
    write_bootstrap,
)
from tests.conftest import _call_tool


@pytest.mark.parametrize(
    ("target_name", "expected_name"),
    [
        ("CLAUDE.md", "CLAUDE.md"),
        ("agents", "AGENTS.md"),
        ("cursor", ".cursorrules"),
    ],
)
def test_write_bootstrap_resolves_supported_target_conventions(
    tmp_path, target_name, expected_name
):
    result = write_bootstrap(tmp_path / target_name)

    target = (tmp_path / expected_name).resolve()
    assert result == {"path": str(target), "changed": True}
    assert target.read_text(encoding="utf-8") == build_bootstrap_block() + "\n"


def test_write_bootstrap_replaces_existing_blocks_without_duplicates(tmp_path):
    target = tmp_path / "AGENTS.md"
    target.write_text(
        "\n".join(
            [
                "# Repo guidance",
                "",
                BEGIN_MARKER,
                "legacy instructions",
                END_MARKER,
                "",
                "Keep this note.",
                "",
                BEGIN_MARKER,
                "duplicate legacy instructions",
                END_MARKER,
                "",
            ]
        ),
        encoding="utf-8",
    )

    first = write_bootstrap(target)
    second = write_bootstrap(target)
    content = target.read_text(encoding="utf-8")

    assert first["changed"] is True
    assert second["changed"] is False
    assert content.count(BEGIN_MARKER) == 1
    assert content.count(END_MARKER) == 1
    assert "# Repo guidance" in content
    assert "Keep this note." in content
    assert "legacy instructions" not in content


def test_write_bootstrap_rejects_incomplete_or_unsupported_targets(tmp_path):
    partial = tmp_path / "CLAUDE.md"
    partial.write_text(f"{BEGIN_MARKER}\npartial block\n", encoding="utf-8")

    with pytest.raises(ValueError, match="incomplete Contextro bootstrap block"):
        write_bootstrap(partial)

    with pytest.raises(ValueError, match="Unsupported bootstrap target"):
        write_bootstrap(tmp_path / "README.md")


def test_skill_prompt_tool_returns_error_for_invalid_target(tmp_path):
    async def run():
        mcp = server_module.create_server()
        return await _call_tool(mcp, "skill_prompt", {"target_path": str(tmp_path / "README.md")})

    result = asyncio.run(run())

    assert "error" in result
    assert "Unsupported bootstrap target" in result["error"]
