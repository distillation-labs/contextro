"""FlashRank two-stage reranker for search result refinement.

Optional dependency — degrades gracefully to passthrough when
flashrank is not installed. Install with: pip install contextro[reranker]
"""

import gc
import logging
from typing import Any, Dict, List, Optional

logger = logging.getLogger(__name__)


class FlashReranker:
    """Reranker using FlashRank for two-stage retrieval.

    Lazy-loads the FlashRank model on first use. Falls back to
    passthrough (return top-k by existing score) when flashrank
    is not installed.
    """

    def __init__(
        self,
        model_name: str = "ms-marco-MiniLM-L-12-v2",
        max_passage_chars: Optional[int] = None,
    ):
        self._model_name = model_name
        self._max_passage_chars = (
            max_passage_chars if max_passage_chars and max_passage_chars > 0 else None
        )
        self._ranker: Optional[Any] = None
        self._available: Optional[bool] = None

    @property
    def available(self) -> bool:
        """Check if flashrank is installed. Caches the result."""
        if self._available is None:
            try:
                import flashrank  # noqa: F401

                self._available = True
            except ImportError:
                self._available = False
                logger.info("FlashRank not installed — reranking disabled")
        return self._available

    def _load_ranker(self):
        """Lazy-load the FlashRank Ranker model."""
        if self._ranker is not None:
            return

        from flashrank import Ranker

        self._ranker = Ranker(model_name=self._model_name)
        logger.info("FlashRank model loaded: %s", self._model_name)

    def rerank(
        self,
        query: str,
        results: List[Dict[str, Any]],
        limit: int = 10,
        max_passage_chars: Optional[int] = None,
    ) -> List[Dict[str, Any]]:
        """Rerank search results using FlashRank.

        If flashrank is unavailable, returns results[:limit] unchanged.

        Args:
            query: Original search query.
            results: Search results to rerank (must have 'text' field).
            limit: Max results to return after reranking.

        Returns:
            Reranked results with updated scores.
        """
        if not results:
            return []

        if not self.available:
            return results[:limit]

        try:
            self._load_ranker()

            from flashrank import RerankRequest

            # Build passages for FlashRank
            passages = []
            passage_chars = (
                max_passage_chars
                if max_passage_chars is not None and max_passage_chars > 0
                else self._max_passage_chars
            )
            for r in results:
                text = r.get("text", r.get("symbol_name", ""))
                if passage_chars is not None:
                    text = text[:passage_chars]
                passages.append({"id": r.get("id", ""), "text": text, "meta": r})

            request = RerankRequest(query=query, passages=passages)
            reranked = self._ranker.rerank(request)
        except Exception as exc:
            logger.warning("FlashRank reranking failed, falling back to passthrough: %s", exc)
            self._available = False
            self._ranker = None
            return results[:limit]

        # Merge rerank scores back into results
        output = []
        for item in reranked[:limit]:
            meta = item.get("meta", {})
            result = dict(meta) if isinstance(meta, dict) else {}
            if not result:
                # meta wasn't a dict — find original by id
                item_id = item.get("id", "")
                for r in results:
                    if r.get("id", "") == item_id:
                        result = dict(r)
                        break
            result["rerank_score"] = float(item.get("score", 0.0))
            result["score"] = float(item.get("score", result.get("score", 0.0)))
            output.append(result)

        return output

    def unload(self):
        """Free the FlashRank model from memory."""
        if self._ranker is not None:
            del self._ranker
            self._ranker = None
            gc.collect()
            logger.info("FlashRank model unloaded")
