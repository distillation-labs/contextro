"""Unit tests for the shared search execution engine."""

from __future__ import annotations

import json
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import MagicMock

import pytest

from contextro_mcp.config import Settings
from contextro_mcp.engines.output_sandbox import OutputSandbox
from contextro_mcp.engines.query_cache import QueryCache
from contextro_mcp.execution.runtime import SearchRuntime, build_search_runtime
from contextro_mcp.execution.search import SearchExecutionEngine, SearchExecutionOptions


class _VectorBackend:
    def __init__(self, results: list[dict]):
        self.results = results
        self.calls: list[tuple[str, int, dict]] = []
        self._embedding_service = MagicMock()
        self._embedding_service.embed.return_value = [0.2, 0.4, 0.6]

    def search(self, query: str, limit: int = 10, **kwargs):
        self.calls.append((query, limit, kwargs))
        return [dict(result) for result in self.results[:limit]]


class _BM25Backend:
    def __init__(self, results: list[dict]):
        self.results = results
        self.calls: list[tuple[str, int, dict]] = []

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
    bm25_backend=None,
    graph_engine=None,
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
    reranker.rerank.side_effect = lambda query, results, limit, **kwargs: results[:limit]
    tracker = MagicMock()

    return SearchRuntime(
        state=SimpleNamespace(_reranker=reranker),
        settings=settings,
        codebase_path=codebase_path,
        codebase_paths=(codebase_path,) if codebase_path else (),
        vector_engine=vector_backend,
        bm25_engine=bm25_backend,
        graph_engine=graph_engine,
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


def test_execute_preview_preserves_bookended_high_signal_results():
    backend = _VectorBackend(
        [
            _result(1),
            _result(2, score=0.85),
            _result(3, score=0.8),
            _result(4, score=0.75),
        ]
    )
    engine = SearchExecutionEngine(
        _runtime(
            backend,
            codebase_path=Path("/repo"),
            sandbox_threshold=1,
            preview_results=2,
        )
    )

    response = engine.execute(
        SearchExecutionOptions(query="symbol", mode="vector", rerank=False, limit=5)
    )

    assert response["sandboxed"] is True
    assert response["full_total"] == 4
    assert [result["name"] for result in response["results"]] == [
        "symbol_1",
        "symbol_2",
    ]


def test_execute_adaptively_trims_high_confidence_results():
    backend = _VectorBackend(
        [
            _result(1, score=0.95),
            _result(2, score=0.6),
            _result(3, score=0.55),
            _result(4, score=0.5),
            _result(5, score=0.45),
        ]
    )
    engine = SearchExecutionEngine(_runtime(backend, codebase_path=Path("/repo")))

    response = engine.execute(
        SearchExecutionOptions(query="AuthManager", mode="vector", rerank=False, limit=5)
    )

    assert response["confidence"] == "high"
    assert response["sandboxed"] is True
    assert response["adaptive_applied"] is True
    assert response["adaptive_limit"] == 3
    assert response["total"] == 2
    assert response["full_total"] == 5
    assert "adaptively trimmed" in response["hint"]

    stored = engine.runtime.output_sandbox.retrieve(response["sandbox_ref"])
    payload = json.loads(stored)
    assert len(payload["results"]) == 5


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


def test_execute_skips_auto_live_grep_for_natural_language_queries(monkeypatch):
    backend = _VectorBackend([_result(1)])
    engine = SearchExecutionEngine(_runtime(backend, codebase_path=Path("/repo")))
    calls: list[tuple[str, int]] = []

    class _LiveGrepStub:
        def __init__(self, workspace_path: str):
            self.workspace_path = workspace_path

        def search(self, query: str, limit: int = 20):
            calls.append((query, limit))
            return []

    monkeypatch.setattr("contextro_mcp.execution.search.LiveGrepEngine", _LiveGrepStub)

    engine.execute(
        SearchExecutionOptions(query="prepare issue worktree", mode="hybrid", rerank=False, limit=3)
    )

    assert calls == []


def test_execute_auto_live_grep_for_single_token_queries(monkeypatch):
    backend = _VectorBackend([_result(1)])
    engine = SearchExecutionEngine(_runtime(backend, codebase_path=Path("/repo")))
    calls: list[tuple[str, int]] = []

    class _LiveGrepStub:
        def __init__(self, workspace_path: str):
            self.workspace_path = workspace_path

        def search(self, query: str, limit: int = 20):
            calls.append((query, limit))
            return []

    monkeypatch.setattr("contextro_mcp.execution.search.LiveGrepEngine", _LiveGrepStub)

    engine.execute(
        SearchExecutionOptions(query="TokenBudget", mode="hybrid", rerank=False, limit=3)
    )

    assert calls == [("TokenBudget", 3)]


def test_hybrid_natural_queries_keep_bm25_and_skip_graph(monkeypatch):
    vector = _VectorBackend([_result(1)])
    bm25 = _BM25Backend([_result(2, score=0.8)])
    runtime = _runtime(
        vector,
        bm25_backend=bm25,
        graph_engine=SimpleNamespace(),
        codebase_path=Path("/repo"),
    )
    engine = SearchExecutionEngine(runtime)
    graph_calls: list[tuple[str, int]] = []

    def _graph_stub(graph_engine, query: str, limit: int = 10):
        graph_calls.append((query, limit))
        return [_result(3, score=0.7)]

    monkeypatch.setattr("contextro_mcp.execution.search.graph_relevance_search", _graph_stub)

    engine.execute(
        SearchExecutionOptions(
            query="discover source files respecting gitignore",
            mode="hybrid",
            rerank=False,
            limit=3,
        )
    )

    assert len(vector.calls) == 1
    assert len(bm25.calls) >= 1
    assert bm25.calls[0][:2] == ("discover source files respecting gitignore", 6)
    assert graph_calls == []


def test_build_search_runtime_uses_cache_and_sandbox_settings():
    settings = Settings()
    settings.search_cache_max_size = 7
    settings.search_cache_similarity_threshold = 0.81
    settings.search_cache_ttl_seconds = 12.5
    settings.search_sandbox_max_entries = 3
    settings.search_sandbox_ttl_seconds = 9.0

    state = SimpleNamespace(codebase_path=Path("/repo"), codebase_paths=[Path("/repo")])
    runtime = build_search_runtime(state, settings)

    assert runtime.query_cache.max_size == 7
    assert runtime.query_cache.similarity_threshold == 0.81
    assert runtime.query_cache.ttl == 12.5
    assert runtime.output_sandbox.max_entries == 3
    assert runtime.output_sandbox.ttl == 9.0


def test_query_cache_prunes_expired_entries_before_lookup():
    cache = QueryCache(ttl=1.0)
    cache.put("auth flow", {"total": 1})
    cache._cache[("", "auth flow")]["timestamp"] -= 5.0

    assert cache.get("auth flow") is None
    assert cache.size == 0


def test_query_cache_matches_camel_and_snake_case_variants():
    cache = QueryCache(similarity_threshold=0.9)
    embedding = [0.9, 0.1, 0.3]
    cache.put("TokenBudgetCache", {"total": 1}, query_embedding=embedding)

    assert cache.get("token_budget cache", query_embedding=embedding) == {"total": 1}


def test_execute_skips_reranker_for_tiny_result_sets():
    backend = _VectorBackend([_result(1)])
    runtime = _runtime(backend, codebase_path=Path("/repo"))
    engine = SearchExecutionEngine(runtime)

    engine.execute(SearchExecutionOptions(query="symbol", mode="vector", rerank=True, limit=5))

    runtime.state._reranker.rerank.assert_not_called()


def test_execute_limits_reranker_candidate_count():
    backend = _VectorBackend([_result(index, score=1.0 - (index * 0.01)) for index in range(1, 21)])
    runtime = _runtime(backend, codebase_path=Path("/repo"))
    runtime.settings.search_rerank_max_candidates = 7
    engine = SearchExecutionEngine(runtime)

    engine.execute(
        SearchExecutionOptions(query="symbol lookup", mode="vector", rerank=True, limit=5)
    )

    rerank_args = runtime.state._reranker.rerank.call_args
    assert rerank_args is not None
    assert len(rerank_args.args[1]) == 7


@pytest.mark.parametrize(
    ("mode", "expected_chars"),
    [
        ("hybrid", 400),
        ("bm25", 400),
        ("vector", 800),
    ],
)
def test_execute_uses_mode_specific_rerank_passage_limit(mode, expected_chars):
    results = [_result(index, score=1.0 - (index * 0.01)) for index in range(1, 6)]
    backend = _VectorBackend(results)
    bm25_backend = _BM25Backend(results) if mode == "bm25" else None
    runtime = _runtime(backend, bm25_backend=bm25_backend, codebase_path=Path("/repo"))
    runtime.settings.search_rerank_max_passage_chars = 800
    runtime.settings.search_rerank_non_vector_passage_chars = 400
    engine = SearchExecutionEngine(runtime)

    engine.execute(SearchExecutionOptions(query="symbol lookup", mode=mode, rerank=True, limit=5))

    rerank_args = runtime.state._reranker.rerank.call_args
    assert rerank_args is not None
    assert rerank_args.kwargs["max_passage_chars"] == expected_chars


def test_output_sandbox_prunes_expired_entries_before_retrieve():
    sandbox = OutputSandbox(ttl=1.0)
    ref_id = sandbox.store("hello world")
    sandbox._store[ref_id]["timestamp"] -= 5.0

    assert sandbox.retrieve(ref_id) is None
    assert sandbox.size == 0
