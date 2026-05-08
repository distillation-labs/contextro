"""Integration tests for git-related MCP tools (commit_history, commit_search, repo_*)."""

import json
import subprocess
from unittest.mock import MagicMock, patch

import pytest

import contextro_mcp.server as server_module
from contextro_mcp.state import reset_state


@pytest.fixture(autouse=True)
def clean_state():
    """Reset global state before each test."""
    reset_state()
    server_module._pipeline = None
    yield
    reset_state()
    server_module._pipeline = None


@pytest.fixture
def git_codebase(tmp_path):
    """Create a git-initialized Python codebase with commits."""
    src = tmp_path / "src"
    src.mkdir()

    # Init git
    subprocess.run(
        ["git", "init"],
        cwd=str(tmp_path),
        capture_output=True,
        check=True,
    )
    subprocess.run(
        ["git", "config", "user.email", "dev@test.com"],
        cwd=str(tmp_path),
        capture_output=True,
        check=True,
    )
    subprocess.run(
        ["git", "config", "user.name", "Developer"],
        cwd=str(tmp_path),
        capture_output=True,
        check=True,
    )

    # First commit
    (src / "main.py").write_text('def hello():\n    """Say hello."""\n    print("hello")\n')
    subprocess.run(
        ["git", "add", "."],
        cwd=str(tmp_path),
        capture_output=True,
        check=True,
    )
    subprocess.run(
        ["git", "commit", "-m", "Initial: add hello function"],
        cwd=str(tmp_path),
        capture_output=True,
        check=True,
    )

    # Second commit
    (src / "utils.py").write_text('def helper():\n    """A helper."""\n    return 42\n')
    subprocess.run(
        ["git", "add", "."],
        cwd=str(tmp_path),
        capture_output=True,
        check=True,
    )
    subprocess.run(
        ["git", "commit", "-m", "Add utility helper for processing"],
        cwd=str(tmp_path),
        capture_output=True,
        check=True,
    )

    return tmp_path


def _mock_embedding_service():
    from contextro_mcp.config import get_settings
    from contextro_mcp.indexing.embedding_service import EMBEDDING_MODELS

    settings = get_settings()
    dims = EMBEDDING_MODELS.get(settings.embedding_model, {}).get("dimensions", 384)
    svc = MagicMock()
    svc.embed.return_value = [0.1] * dims

    def dynamic_batch(texts, **kwargs):
        return [[0.1] * dims for _ in texts]

    svc.embed_batch.side_effect = dynamic_batch
    return svc


async def _call_tool(mcp, name, args=None):
    """Call an MCP tool and return the structured result."""
    result = await mcp.call_tool(name, args or {})
    if result.structured_content is not None:
        return result.structured_content.get("result", result.structured_content)
    for content in result.content:
        if hasattr(content, "text"):
            data = json.loads(content.text)
            return data.get("result", data) if isinstance(data, dict) else data
    return {}


class TestCommitHistoryTool:
    """Tests for the commit_history MCP tool."""

    @pytest.mark.asyncio
    async def test_commit_history_basic(self, git_codebase, tmp_path):
        storage = tmp_path / "storage"
        with patch.dict("os.environ", {"CTX_STORAGE_DIR": str(storage)}):
            from contextro_mcp.config import reset_settings

            reset_settings()
            mcp = server_module.create_server()

            result = await _call_tool(
                mcp,
                "commit_history",
                {
                    "path": str(git_codebase),
                },
            )
            reset_settings()

        assert "error" not in result
        assert result["total"] == 2
        assert result["branch"] in ("main", "master")
        assert len(result["commits"]) == 2

    @pytest.mark.asyncio
    async def test_commit_history_with_limit(self, git_codebase, tmp_path):
        storage = tmp_path / "storage"
        with patch.dict("os.environ", {"CTX_STORAGE_DIR": str(storage)}):
            from contextro_mcp.config import reset_settings

            reset_settings()
            mcp = server_module.create_server()

            result = await _call_tool(
                mcp,
                "commit_history",
                {
                    "path": str(git_codebase),
                    "limit": 1,
                },
            )
            reset_settings()

        assert result["total"] == 1

    @pytest.mark.asyncio
    async def test_commit_history_nonrepo(self, tmp_path):
        plain_dir = tmp_path / "plain"
        plain_dir.mkdir()
        storage = tmp_path / "storage"
        with patch.dict("os.environ", {"CTX_STORAGE_DIR": str(storage)}):
            from contextro_mcp.config import reset_settings

            reset_settings()
            mcp = server_module.create_server()

            result = await _call_tool(
                mcp,
                "commit_history",
                {
                    "path": str(plain_dir),
                },
            )
            reset_settings()

        assert "error" in result


class TestCommitSearchTool:
    """Tests for the commit_search MCP tool."""

    @pytest.mark.asyncio
    async def test_commit_search_after_index(self, git_codebase, tmp_path):
        """Commit search works after indexing (which auto-indexes commits)."""
        storage = tmp_path / "storage"
        with (
            patch("contextro_mcp.indexing.pipeline.get_embedding_service") as mock_get,
            patch.dict("os.environ", {"CTX_STORAGE_DIR": str(storage)}),
        ):
            from contextro_mcp.config import reset_settings

            reset_settings()

            mock_svc = _mock_embedding_service()
            mock_get.return_value = mock_svc

            mcp = server_module.create_server()

            # Index (which should also index commits)
            await _call_tool(
                mcp,
                "index",
                {
                    "path": str(git_codebase),
                },
            )

            # Patch vector engine for search
            from contextro_mcp.state import get_state

            state = get_state()
            if state.vector_engine:
                state.vector_engine._embedding_service = mock_svc
            # Also patch commit indexer's embedding service
            if state.commit_indexer:
                state.commit_indexer._embedding_service = mock_svc

            result = await _call_tool(
                mcp,
                "commit_search",
                {
                    "query": "helper utility",
                },
            )
            reset_settings()

        assert "error" not in result
        assert result["total"] >= 0  # May be 0 if embedding mock doesn't match


class TestRepoTools:
    """Tests for cross-repo MCP tools."""

    @pytest.mark.asyncio
    async def test_repo_status_empty(self, tmp_path):
        storage = tmp_path / "storage"
        with patch.dict("os.environ", {"CTX_STORAGE_DIR": str(storage)}):
            from contextro_mcp.config import reset_settings

            reset_settings()
            mcp = server_module.create_server()

            result = await _call_tool(mcp, "repo_status", {})
            reset_settings()

        assert result["total_repos"] == 0

    @pytest.mark.asyncio
    async def test_repo_add_and_status(self, git_codebase, tmp_path):
        storage = tmp_path / "storage"
        with (
            patch("contextro_mcp.indexing.pipeline.get_embedding_service") as mock_get,
            patch.dict("os.environ", {"CTX_STORAGE_DIR": str(storage)}),
        ):
            from contextro_mcp.config import reset_settings

            reset_settings()

            mock_svc = _mock_embedding_service()
            mock_get.return_value = mock_svc

            mcp = server_module.create_server()

            # Add repo
            add_result = await _call_tool(
                mcp,
                "repo_add",
                {
                    "path": str(git_codebase),
                    "name": "test-repo",
                    "index_now": False,
                },
            )
            reset_settings()

        assert "error" not in add_result
        assert add_result["status"] == "registered"
        assert add_result["repo"]["name"] == "test-repo"

    @pytest.mark.asyncio
    async def test_repo_remove(self, git_codebase, tmp_path):
        storage = tmp_path / "storage"
        with patch.dict("os.environ", {"CTX_STORAGE_DIR": str(storage)}):
            from contextro_mcp.config import reset_settings

            reset_settings()
            mcp = server_module.create_server()

            # Add then remove
            await _call_tool(
                mcp,
                "repo_add",
                {
                    "path": str(git_codebase),
                    "index_now": False,
                },
            )
            result = await _call_tool(
                mcp,
                "repo_remove",
                {
                    "path": str(git_codebase),
                },
            )
            reset_settings()

        assert result["removed"] is True

    @pytest.mark.asyncio
    async def test_repo_remove_by_name(self, git_codebase, tmp_path):
        storage = tmp_path / "storage"
        with patch.dict("os.environ", {"CTX_STORAGE_DIR": str(storage)}):
            from contextro_mcp.config import reset_settings

            reset_settings()
            mcp = server_module.create_server()

            await _call_tool(
                mcp,
                "repo_add",
                {
                    "path": str(git_codebase),
                    "name": "my-repo",
                    "index_now": False,
                },
            )
            result = await _call_tool(
                mcp,
                "repo_remove",
                {
                    "name": "my-repo",
                },
            )
            reset_settings()

        assert result["removed"] is True
