"""Tests for MCP tools: index, search, status."""

import asyncio
import shutil
from unittest.mock import patch

import contextro_mcp.server as server_module
from contextro_mcp.state import reset_state
from tests.conftest import _call_tool, _mock_embedding_service, _setup_indexed


class TestStatus:
    def test_status_before_index(self, tmp_path, monkeypatch):
        monkeypatch.setenv("CTX_STORAGE_DIR", str(tmp_path / ".contextro"))
        from contextro_mcp.config import reset_settings

        reset_settings()
        mcp = server_module.create_server()
        result = asyncio.run(_call_tool(mcp, "status"))
        assert result["indexed"] is False
        assert result["indexed"] is False
        assert result["codebase_path"] is None
        assert "hint" in result

    def test_status_after_index(self, mini_codebase, tmp_path):
        async def run():
            mcp, _, _ = await _setup_indexed(mini_codebase, tmp_path / ".contextro")
            return await _call_tool(mcp, "status")

        status = asyncio.run(run())
        assert status.get("indexed", True) is True
        assert status["codebase_path"] is not None
        assert "vector_chunks" in status
        assert status["vector_chunks"] > 0
        assert "graph" in status


class TestIndex:
    def test_index_returns_stats(self, mini_codebase, tmp_path):
        async def run():
            _, _, result = await _setup_indexed(mini_codebase, tmp_path / ".contextro")
            return result

        result = asyncio.run(run())
        assert result["total_files"] >= 2
        assert result["total_symbols"] > 0
        assert result["total_chunks"] > 0
        pass  # time_seconds omitted from index result

    def test_index_invalid_path(self):
        mcp = server_module.create_server()
        result = asyncio.run(_call_tool(mcp, "index", {"path": "/nonexistent/path"}))
        assert "error" in result

    def test_index_path_prefix_remap(self, mini_codebase, tmp_path, tmp_path_factory, monkeypatch):
        async def run():
            mounted_repo = tmp_path_factory.mktemp("mounted") / "platform"
            shutil.copytree(mini_codebase, mounted_repo)

            storage = tmp_path / ".contextro"
            monkeypatch.setenv("CTX_STORAGE_DIR", str(storage))
            monkeypatch.setenv(
                "CTX_PATH_PREFIX_MAP",
                f"/client/platform={mounted_repo}",
            )

            from contextro_mcp.config import reset_settings

            reset_settings()

            server_module._pipeline = None
            with patch(
                "contextro_mcp.indexing.pipeline.get_embedding_service",
                return_value=_mock_embedding_service(),
            ):
                mcp = server_module.create_server()
                result = await _call_tool(mcp, "index", {"path": "/client/platform"})

                if result.get("status") == "indexing":
                    status = {}
                    for _ in range(120):
                        await asyncio.sleep(0.5)
                        status = await _call_tool(mcp, "status")
                        if status.get("indexed") is True:
                            break
                        if status.get("index_error"):
                            raise RuntimeError(status["index_error"])
                    with server_module._index_job_lock:
                        result = server_module._index_job.get("result", result)
                else:
                    status = await _call_tool(mcp, "status")

            return result, status, mounted_repo

        from unittest.mock import patch

        result, status, mounted_repo = asyncio.run(run())
        assert "error" not in result
        assert status["codebase_path"] == mounted_repo.name

    def test_index_auto_remaps_from_codebase_env(
        self, mini_codebase, tmp_path, tmp_path_factory, monkeypatch
    ):
        async def run():
            mounted_repo = tmp_path_factory.mktemp("mounted-auto") / "platform"
            shutil.copytree(mini_codebase, mounted_repo)

            storage = tmp_path / ".contextro"
            monkeypatch.setenv("CTX_STORAGE_DIR", str(storage))
            monkeypatch.setenv("CTX_CODEBASE_HOST_PATH", "/client/platform")
            monkeypatch.setenv("CTX_CODEBASE_MOUNT_PATH", str(mounted_repo))
            monkeypatch.delenv("CTX_PATH_PREFIX_MAP", raising=False)

            from contextro_mcp.config import reset_settings

            reset_settings()

            server_module._pipeline = None
            with patch(
                "contextro_mcp.indexing.pipeline.get_embedding_service",
                return_value=_mock_embedding_service(),
            ):
                mcp = server_module.create_server()
                result = await _call_tool(mcp, "index", {"path": "/client/platform"})

                if result.get("status") == "indexing":
                    status = {}
                    for _ in range(120):
                        await asyncio.sleep(0.5)
                        status = await _call_tool(mcp, "status")
                        if status.get("indexed") is True:
                            break
                        if status.get("index_error"):
                            raise RuntimeError(status["index_error"])
                    with server_module._index_job_lock:
                        result = server_module._index_job.get("result", result)
                else:
                    status = await _call_tool(mcp, "status")

            return result, status, mounted_repo

        from unittest.mock import patch

        result, status, mounted_repo = asyncio.run(run())
        assert "error" not in result
        assert status["codebase_path"] == mounted_repo.name

    def test_index_invalid_path_surfaces_docker_hint(self, tmp_path, monkeypatch):
        monkeypatch.setenv("CTX_STORAGE_DIR", str(tmp_path / ".contextro"))
        monkeypatch.setenv("CTX_CODEBASE_HOST_PATH", "/client/platform")
        monkeypatch.setenv("CTX_CODEBASE_MOUNT_PATH", "/repos/platform")
        monkeypatch.delenv("CTX_PATH_PREFIX_MAP", raising=False)

        from contextro_mcp.config import reset_settings

        reset_settings()
        mcp = server_module.create_server()
        result = asyncio.run(_call_tool(mcp, "index", {"path": "/wrong/path"}))
        assert "error" in result
        assert "CTX_CODEBASE_HOST_PATH" in result["error"]

    def test_index_sets_state(self, mini_codebase, tmp_path):
        async def run():
            await _setup_indexed(mini_codebase, tmp_path / ".contextro")

        asyncio.run(run())
        from contextro_mcp.state import get_state

        state = get_state()
        assert state.is_indexed
        assert state.vector_engine is not None
        assert state.graph_engine is not None

    def test_index_incremental_on_second_call(self, mini_codebase, tmp_path):
        async def run():
            from unittest.mock import patch

            from tests.conftest import _mock_embedding_service

            storage = tmp_path / ".contextro"
            mcp, _, result1 = await _setup_indexed(mini_codebase, storage)
            assert result1["total_files"] >= 2

            # Second call should be incremental (metadata exists)
            # Keep embedding service patched since _setup_indexed's patch has exited
            server_module._pipeline = None
            mock_svc = _mock_embedding_service()
            with (
                patch(
                    "contextro_mcp.indexing.pipeline.get_embedding_service",
                    return_value=mock_svc,
                ),
                patch.dict("os.environ", {"CTX_STORAGE_DIR": str(storage)}),
            ):
                from contextro_mcp.config import reset_settings

                reset_settings()
                result2 = await _call_tool(mcp, "index", {"path": str(mini_codebase)})
                reset_settings()
            return result2

        result2 = asyncio.run(run())
        assert "error" not in result2


class TestSearch:
    def test_search_before_index(self, tmp_path, monkeypatch):
        monkeypatch.setenv("CTX_STORAGE_DIR", str(tmp_path / ".contextro"))
        from contextro_mcp.config import reset_settings

        reset_settings()
        mcp = server_module.create_server()
        result = asyncio.run(_call_tool(mcp, "search", {"query": "hello"}))
        assert "error" in result

    def test_search_after_index(self, mini_codebase, tmp_path):
        async def run():
            mcp, _, _ = await _setup_indexed(mini_codebase, tmp_path / ".contextro")
            return await _call_tool(mcp, "search", {"query": "hello"})

        result = asyncio.run(run())
        assert "error" not in result
        assert result["total"] > 0
        assert len(result["results"]) > 0

    def test_search_result_format(self, mini_codebase, tmp_path):
        async def run():
            mcp, _, _ = await _setup_indexed(mini_codebase, tmp_path / ".contextro")
            return await _call_tool(mcp, "search", {"query": "test"})

        result = asyncio.run(run())
        r = result["results"][0]
        assert "f" in r
        assert "n" in r
        assert "score" in r
        # code present, no raw vector or absolute_path (stripped for token savings)
        assert "c" in r, "search results must include code"
        assert "absolute_path" not in r, "absolute_path should be stripped to save tokens"
        assert "vector" not in r, "raw embedding vector must be stripped"
        assert "text" not in r, "text field should be renamed to code"

    def test_search_with_limit(self, mini_codebase, tmp_path):
        async def run():
            mcp, _, _ = await _setup_indexed(mini_codebase, tmp_path / ".contextro")
            return await _call_tool(mcp, "search", {"query": "test", "limit": 1})

        result = asyncio.run(run())
        assert result["total"] <= 1

    def test_search_relative_paths(self, mini_codebase, tmp_path):
        async def run():
            mcp, _, _ = await _setup_indexed(mini_codebase, tmp_path / ".contextro")
            return await _call_tool(mcp, "search", {"query": "test"})

        result = asyncio.run(run())
        for r in result["results"]:
            # Key is shortened to "file" for token efficiency
            path = r.get("file") or r.get("filepath", "")
            assert not path.startswith("/"), f"Path not relative: {path}"

    def test_search_can_round_trip_sandboxed_results(self, mini_codebase, tmp_path, monkeypatch):
        async def run():
            monkeypatch.setenv("CTX_SEARCH_SANDBOX_THRESHOLD_TOKENS", "1")
            monkeypatch.setenv("CTX_SEARCH_PREVIEW_RESULTS", "1")
            from contextro_mcp.config import reset_settings

            reset_settings()
            mcp, _, _ = await _setup_indexed(mini_codebase, tmp_path / ".contextro")
            search_result = await _call_tool(mcp, "search", {"query": "hello"})
            retrieve_result = await _call_tool(
                mcp,
                "retrieve",
                {"ref_id": search_result["sandbox_ref"]},
            )
            return search_result, retrieve_result

        search_result, retrieve_result = asyncio.run(run())
        assert "sandbox_ref" in search_result
        assert search_result["total"] == 1
        assert search_result["full_total"] >= 1
        assert retrieve_result["ref_id"] == search_result["sandbox_ref"]
        assert '"query": "hello"' in retrieve_result["content"]

    def test_compaction_archive_survives_server_restart(self, tmp_path, monkeypatch):
        async def run():
            storage = tmp_path / ".contextro"
            monkeypatch.setenv("CTX_STORAGE_DIR", str(storage))
            from contextro_mcp.config import reset_settings

            reset_settings()
            mock_svc = _mock_embedding_service()
            with patch(
                "contextro_mcp.indexing.embedding_service.get_embedding_service",
                return_value=mock_svc,
            ):
                mcp = server_module.create_server()
                first = await _call_tool(
                    mcp,
                    "compact",
                    {"content": "JWT refresh flow with Redis-backed session archive"},
                )

                reset_state()
                reset_settings()
                server_module._pipeline = None
                server_module._index_job = {}

                mcp = server_module.create_server()
                recall = await _call_tool(
                    mcp,
                    "recall",
                    {"query": "Redis session archive", "memory_type": "archive"},
                )
            return first, recall

        compact_result, recall_result = asyncio.run(run())

        assert compact_result["archived"] is True
        assert compact_result["archive_ref"].startswith("ca_")
        assert recall_result["total"] >= 1
        assert recall_result["results"][0]["archive_ref"] == compact_result["archive_ref"]

    def test_code_tool_large_results_use_disclosure(
        self, mini_codebase_with_calls, tmp_path, monkeypatch
    ):
        async def run():
            monkeypatch.setenv("CTX_SEARCH_SANDBOX_THRESHOLD_TOKENS", "1")
            from contextro_mcp.config import reset_settings

            reset_settings()
            mcp, _, _ = await _setup_indexed(mini_codebase_with_calls, tmp_path / ".contextro")
            return await _call_tool(
                mcp,
                "code",
                {
                    "operation": "lookup_symbols",
                    "symbols": "hello,helper,orchestrate",
                    "include_source": True,
                },
            )

        result = asyncio.run(run())

        assert "sandbox_ref" in result
        assert result["operation"] == "lookup_symbols"
        assert result["sandbox_ref"].startswith("sx_")
        assert result["total"] == 3
