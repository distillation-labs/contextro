"""Unit tests for the shared search execution engine."""

from __future__ import annotations

import json
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import MagicMock

import pytest

from contextia_mcp.config import Settings
from contextia_mcp.engines.output_sandbox import OutputSandbox
from contextia_mcp.engines.query_cache import QueryCache
from contextia_mcp.execution.runtime import SearchRuntime
from contextia_mcp.execution.search import SearchExecutionEngine, SearchExecutionOptions


class _VectorBackend:
    def __init__(self, results: list[dict]):
        self.results = results
        self.calls: list[tuple[str, int, dict]] = []
        self._embedding_service = MagicMock()
        self._embedding_service.embed.return_value = [0.2, 0.4, 0.6]

    def search(self, query: str, limit: int = 10, **kwargs):
        self.calls.append((query, limit, kwargs))
        return [dict(result) for result in self.results[:limit]]


def _result(index: int, *, score: float = 0.9) -> dict:
    return {
        "filepath": f"/repo/src/file_{index}.py",
        "line_start": index,
        "line_end": index + 4,
        "symbol_name": f"symbol_{index}",
        "score": score,
        "text": "def symbol():\n" + ("    value = 'x'\n" * 60),
        "language": "python",
    }


def _runtime(
    vector_backend: _VectorBackend,
    *,
    codebase_path: Path | None = None,
    sandbox_threshold: int = 10_000,
    preview_results: int = 2,
    query_aware_compression: bool = True,
) -> SearchRuntime:
    settings = Settings()
    settings.relevance_threshold = 0.0
    settings.search_sandbox_threshold_tokens = sandbox_threshold
    settings.search_preview_results = preview_results
    settings.search_preview_code_chars = 40
    settings.search_query_aware_compression = query_aware_compression

    reranker = MagicMock()
    reranker.rerank.side_effect = lambda query, results, limit: results[:limit]
    tracker = MagicMock()

    return SearchRuntime(
        state=SimpleNamespace(_reranker=reranker),
        settings=settings,
        codebase_path=codebase_path,
        codebase_paths=(codebase_path,) if codebase_path else (),
        vector_engine=vector_backend,
        bm25_engine=None,
        graph_engine=None,
        query_cache=QueryCache(similarity_threshold=0.95),
        output_sandbox=OutputSandbox(),
        session_tracker=tracker,
    )


@pytest.mark.parametrize(
    ("first", "second"),
    [
        (
            SearchExecutionOptions(query="auth flow", mode="vector", rerank=False),
            SearchExecutionOptions(
                query="auth flow",
                mode="vector",
                rerank=False,
                context_budget=10,
            ),
        ),
        (
            SearchExecutionOptions(query="auth flow", mode="vector", rerank=False),
            SearchExecutionOptions(query="auth flow", mode="vector", rerank=True),
        ),
        (
            SearchExecutionOptions(query="auth flow", mode="vector", rerank=False),
            SearchExecutionOptions(query="auth flow", mode="vector", rerank=False, live_grep=True),
        ),
    ],
)
def test_search_cache_isolated_by_option_namespace(first, second):
    backend = _VectorBackend([_result(1)])
    engine = SearchExecutionEngine(_runtime(backend))

    engine.execute(first)
    engine.execute(second)

    assert len(backend.calls) == 2
    assert engine.runtime.query_cache.size == 2


def test_execute_sandboxes_large_responses_and_stores_full_results():
    backend = _VectorBackend([_result(1), _result(2, score=0.8)])
    engine = SearchExecutionEngine(
        _runtime(
            backend,
            codebase_path=Path("/repo"),
            sandbox_threshold=1,
            preview_results=1,
        )
    )

    response = engine.execute(
        SearchExecutionOptions(query="symbol", mode="vector", rerank=False, limit=5)
    )

    assert response["sandboxed"] is True
    assert response["sandbox_ref"].startswith("sx_")
    assert response["total"] == 1
    assert response["full_total"] == 2
    assert "retrieve()" in response["hint"]

    stored = engine.runtime.output_sandbox.retrieve(response["sandbox_ref"])
    payload = json.loads(stored)
    assert payload["query"] == "symbol"
    assert len(payload["results"]) == 2


def test_execute_keeps_query_focal_lines_in_compressed_code():
    body = "\n".join(
        [
            "def symbol():",
            "    first_value = 1",
            "    second_value = 2",
            "    filler_before = compute_first()",
            "    filler_before += 1",
            "    target_token = validate_session_token(token)",
            "    if not target_token:",
            "        raise ValueError('missing token')",
            "    return target_token",
            "    filler_after = cleanup()",
            "    filler_after += 1",
        ]
    )
    backend = _VectorBackend([{**_result(1), "text": body}])
    engine = SearchExecutionEngine(_runtime(backend, codebase_path=Path("/repo")))

    response = engine.execute(
        SearchExecutionOptions(
            query="validate session token",
            mode="vector",
            rerank=False,
            limit=1,
        )
    )

    code = response["results"][0]["code"]
    assert "target_token = validate_session_token(token)" in code
    assert "return target_token" in code
    assert "first_value = 1" not in code


def test_execute_trims_tail_results_more_aggressively():
    backend = _VectorBackend([_result(1), _result(2, score=0.8), _result(3, score=0.7)])
    engine = SearchExecutionEngine(_runtime(backend, codebase_path=Path("/repo")))

    response = engine.execute(
        SearchExecutionOptions(query="nonexistent", mode="vector", rerank=False, limit=3)
    )

    codes = [result["code"] for result in response["results"]]
    assert len(codes[0]) > len(codes[1]) > len(codes[2])


def test_execute_respects_configurable_code_budgets():
    backend = _VectorBackend([_result(1), _result(2, score=0.8), _result(3, score=0.7)])
    runtime = _runtime(backend, codebase_path=Path("/repo"), query_aware_compression=False)
    runtime.settings.search_code_budget_top_chars = 40
    runtime.settings.search_code_budget_second_chars = 30
    runtime.settings.search_code_budget_tail_chars = 20
    engine = SearchExecutionEngine(runtime)

    response = engine.execute(
        SearchExecutionOptions(query="nonexistent", mode="vector", rerank=False, limit=3)
    )

    codes = [result["code"] for result in response["results"]]
    assert len(codes[0]) <= 41
    assert len(codes[1]) <= 31
    assert len(codes[2]) <= 21
