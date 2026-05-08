"""Tests for FlashRank reranker."""

from unittest.mock import MagicMock, patch

from contextro_mcp.engines.reranker import FlashReranker


def _make_result(id_: str, text: str = "test", score: float = 0.5):
    return {"id": id_, "text": text, "score": score, "symbol_name": f"func_{id_}"}


class TestAvailability:
    def test_available_when_installed(self):
        reranker = FlashReranker()
        with patch.dict("sys.modules", {"flashrank": MagicMock()}):
            reranker._available = None  # Reset cache
            assert reranker.available is True

    def test_unavailable_when_not_installed(self):
        reranker = FlashReranker()
        with patch.dict("sys.modules", {"flashrank": None}):
            reranker._available = None
            # When module is None in sys.modules, import raises ImportError
            assert reranker.available is False

    def test_availability_cached(self):
        reranker = FlashReranker()
        reranker._available = True
        assert reranker.available is True
        reranker._available = False
        assert reranker.available is False


class TestPassthrough:
    def test_passthrough_when_unavailable(self):
        reranker = FlashReranker()
        reranker._available = False
        results = [_make_result("a"), _make_result("b"), _make_result("c")]
        output = reranker.rerank("query", results, limit=2)
        assert len(output) == 2
        assert output[0]["id"] == "a"
        assert output[1]["id"] == "b"

    def test_passthrough_preserves_scores(self):
        reranker = FlashReranker()
        reranker._available = False
        results = [_make_result("a", score=0.9)]
        output = reranker.rerank("query", results)
        assert output[0]["score"] == 0.9

    def test_empty_results(self):
        reranker = FlashReranker()
        reranker._available = False
        assert reranker.rerank("query", []) == []


class TestReranking:
    def test_rerank_with_mock_flashrank(self):
        """Test reranking with mocked FlashRank."""
        mock_flashrank = MagicMock()
        mock_rerank_request = MagicMock()
        mock_flashrank.RerankRequest = mock_rerank_request

        with patch.dict("sys.modules", {"flashrank": mock_flashrank}):
            reranker = FlashReranker()
            reranker._available = True

            mock_ranker = MagicMock()
            mock_ranker.rerank.return_value = [
                {"id": "b", "score": 0.95, "text": "t2", "meta": _make_result("b", score=0.3)},
                {"id": "a", "score": 0.80, "text": "t1", "meta": _make_result("a", score=0.9)},
            ]
            reranker._ranker = mock_ranker

            results = [_make_result("a", score=0.9), _make_result("b", score=0.3)]
            output = reranker.rerank("query", results, limit=10)

        assert len(output) == 2
        assert output[0]["id"] == "b"  # Reranked to top
        assert output[0]["rerank_score"] == 0.95

    def test_rerank_limit(self):
        mock_flashrank = MagicMock()
        mock_flashrank.RerankRequest = MagicMock()

        with patch.dict("sys.modules", {"flashrank": mock_flashrank}):
            reranker = FlashReranker()
            reranker._available = True

            mock_ranker = MagicMock()
            mock_ranker.rerank.return_value = [
                {"id": "a", "score": 0.9, "meta": _make_result("a")},
                {"id": "b", "score": 0.8, "meta": _make_result("b")},
                {"id": "c", "score": 0.7, "meta": _make_result("c")},
            ]
            reranker._ranker = mock_ranker

            results = [_make_result("a"), _make_result("b"), _make_result("c")]
            output = reranker.rerank("query", results, limit=2)

        assert len(output) == 2

    def test_rerank_truncates_passages_when_configured(self):
        mock_flashrank = MagicMock()
        mock_request = MagicMock()
        mock_flashrank.RerankRequest = mock_request

        with patch.dict("sys.modules", {"flashrank": mock_flashrank}):
            reranker = FlashReranker(max_passage_chars=5)
            reranker._available = True

            mock_ranker = MagicMock()
            mock_ranker.rerank.return_value = [
                {"id": "a", "score": 0.9, "meta": _make_result("a", text="abcdefghij")},
            ]
            reranker._ranker = mock_ranker

            reranker.rerank("query", [_make_result("a", text="abcdefghij")], limit=1)

        assert mock_request.call_count == 1
        _, kwargs = mock_request.call_args
        assert kwargs["passages"][0]["text"] == "abcde"

    def test_rerank_override_uses_call_specific_passage_limit(self):
        mock_flashrank = MagicMock()
        mock_request = MagicMock()
        mock_flashrank.RerankRequest = mock_request

        with patch.dict("sys.modules", {"flashrank": mock_flashrank}):
            reranker = FlashReranker(max_passage_chars=10)
            reranker._available = True

            mock_ranker = MagicMock()
            mock_ranker.rerank.return_value = [
                {"id": "a", "score": 0.9, "meta": _make_result("a", text="abcdefghij")},
            ]
            reranker._ranker = mock_ranker

            reranker.rerank(
                "query",
                [_make_result("a", text="abcdefghij")],
                limit=1,
                max_passage_chars=4,
            )

        _, kwargs = mock_request.call_args
        assert kwargs["passages"][0]["text"] == "abcd"


class TestUnload:
    def test_unload_frees_ranker(self):
        reranker = FlashReranker()
        reranker._ranker = MagicMock()
        reranker.unload()
        assert reranker._ranker is None

    def test_unload_when_not_loaded(self):
        reranker = FlashReranker()
        reranker.unload()  # Should not raise
        assert reranker._ranker is None
