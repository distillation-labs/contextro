"""Shared search execution engine for Contextro."""

from __future__ import annotations

import logging
import re as _re
from dataclasses import dataclass
from typing import Any

from contextro_mcp.engines.fusion import ReciprocalRankFusion, graph_relevance_search
from contextro_mcp.engines.live_grep import LiveGrepEngine
from contextro_mcp.engines.reranker import FlashReranker
from contextro_mcp.execution.compaction import (
    CodeCompressionBudgets,
    SearchResultCompactor,
)
from contextro_mcp.execution.response_policy import SearchResponsePolicy, SearchResponseSettings
from contextro_mcp.execution.runtime import SearchRuntime

logger = logging.getLogger(__name__)

_SYMBOL_RE = _re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
_CAMEL_RE = _re.compile(r"[a-z][A-Z]|[A-Z]{2,}[a-z]")


def classify_query(query: str) -> str:
    """Classify query intent to route to the most relevant retrievers.

    Returns:
        'symbol'   — single identifier (PascalCase/snake_case/camelCase) → BM25 + graph only
        'natural'  — multi-word natural language → vector + graph only
        'hybrid'   — ambiguous or mixed → all retrievers (default)
    """
    q = query.strip()
    if not q:
        return "hybrid"
    # Single token that looks like a code identifier
    if _SYMBOL_RE.match(q) or (len(q.split()) == 1 and (_CAMEL_RE.search(q) or "_" in q)):
        return "symbol"
    # Multi-word natural language (5+ words, no identifier-like tokens)
    words = q.split()
    if len(words) >= 5 and not any(_CAMEL_RE.search(w) or "_" in w for w in words):
        return "natural"
    return "hybrid"


@dataclass(frozen=True, slots=True)
class SearchExecutionOptions:
    """Normalized search options passed into the shared execution engine."""

    query: str
    limit: int = 10
    language: str = ""
    symbol_type: str = ""
    mode: str = "hybrid"
    rerank: bool = True
    live_grep: bool = False
    context_budget: int = 0


class SearchExecutionEngine:
    """Provider-agnostic execution engine for Contextro search flows."""

    def __init__(self, runtime: SearchRuntime):
        self.runtime = runtime
        self.state = runtime.state
        self.settings = runtime.settings
        self._compactor = SearchResultCompactor(
            codebase_paths=runtime.codebase_paths,
            budgets=CodeCompressionBudgets.from_settings(runtime.settings),
        )
        self._response_policy = SearchResponsePolicy(
            output_sandbox=runtime.output_sandbox,
            settings=SearchResponseSettings.from_settings(runtime.settings),
        )

    def execute(self, options: SearchExecutionOptions) -> dict[str, Any]:
        """Run the shared hybrid-search flow and return a token-aware response."""

        limit = max(1, min(options.limit, 100))
        cache_namespace = self._cache_namespace(options, limit)

        cached = self.runtime.query_cache.get(options.query, namespace=cache_namespace)
        if cached is not None:
            return cached

        query_embedding = self._query_embedding(options)
        if query_embedding is not None:
            cached = self.runtime.query_cache.get(
                options.query,
                query_embedding=query_embedding,
                namespace=cache_namespace,
            )
            if cached is not None:
                return cached

        filters = self._search_filters(options)
        route = classify_query(options.query) if options.mode == "hybrid" else "hybrid"
        ranked_lists, engines_used = self._collect_ranked_lists(
            options,
            limit=limit,
            filters=filters,
            query_embedding=query_embedding,
            route=route,
        )

        results = self._fuse_results(ranked_lists, route=route)
        self._annotate_match_sources(results)
        results = self._rerank_results(options, results, limit)
        results = self._filter_relevance(results)
        results = self._apply_same_file_diversity(results)
        results = self._dedupe_overlapping_results(results)
        results = self._apply_bm25_fallback(options, filters, results, limit, engines_used)
        results = self._apply_live_grep(options, results, limit, engines_used)
        results = self._compact_results(results, options.query)
        results = self._apply_bookend_ordering(results)

        confidence = self._calculate_confidence(results)
        display_results, adaptive_applied = self._apply_adaptive_result_count(
            options,
            results,
            confidence,
        )
        response = self._response_policy.build(
            query=options.query,
            results=display_results,
            full_results=results,
            confidence=confidence,
            context_budget=options.context_budget,
            adaptive_applied=adaptive_applied,
        )

        self.runtime.query_cache.put(
            options.query,
            response,
            query_embedding=query_embedding,
            namespace=cache_namespace,
        )
        self.runtime.session_tracker.track_search(
            options.query,
            response.get("full_total", response["total"]),
        )
        return response

    @staticmethod
    def _cache_namespace(options: SearchExecutionOptions, limit: int) -> str:
        return (
            f"{limit}|{options.language}|{options.symbol_type}|"
            f"{options.mode}|{options.rerank}|{options.live_grep}|{options.context_budget}"
        )

    def _query_embedding(self, options: SearchExecutionOptions) -> list[float] | None:
        if options.mode not in ("hybrid", "vector") or not self.runtime.vector_engine:
            return None
        if options.mode == "hybrid" and classify_query(options.query) == "symbol":
            return None

        try:
            return self.runtime.vector_engine._embedding_service.embed(options.query, is_query=True)
        except Exception as exc:
            logger.debug("Query embedding failed: %s", exc)
            return None

    @staticmethod
    def _search_filters(options: SearchExecutionOptions) -> dict[str, str]:
        filters: dict[str, str] = {}
        if options.language:
            filters["language"] = options.language
        if options.symbol_type:
            filters["symbol_type"] = options.symbol_type
        return filters

    def _collect_ranked_lists(
        self,
        options: SearchExecutionOptions,
        limit: int,
        filters: dict[str, str],
        query_embedding: list[float] | None,
        route: str,
    ) -> tuple[dict[str, list[dict[str, Any]]], list[str]]:
        ranked_lists: dict[str, list[dict[str, Any]]] = {}
        engines_used: list[str] = []
        overfetch = limit * 2

        # Route query to skip unnecessary retrievers.
        # Natural-language queries still benefit from keyword retrieval, but
        # graph-centrality matches tend to add noise for prose-like searches.
        use_vector = options.mode in ("hybrid", "vector") and route != "symbol"
        use_bm25 = options.mode in ("hybrid", "bm25")
        use_graph = options.mode == "hybrid" and route != "natural"

        if use_vector and self.runtime.vector_engine:
            try:
                vector_kwargs = dict(filters)
                if query_embedding is not None:
                    vector_kwargs["query_vector"] = query_embedding
                vector_results = self.runtime.vector_engine.search(
                    options.query,
                    limit=overfetch,
                    **vector_kwargs,
                )
                ranked_lists["vector"] = vector_results
                engines_used.append("vector")
            except Exception as exc:
                logger.warning("Vector search failed: %s", exc)

        if use_bm25 and self.runtime.bm25_engine:
            try:
                bm25_results = self.runtime.bm25_engine.search(
                    options.query,
                    limit=overfetch,
                    **filters,
                )
                if bm25_results:
                    ranked_lists["bm25"] = bm25_results
                    engines_used.append("bm25")
            except Exception as exc:
                logger.warning("BM25 search failed: %s", exc)

        if use_graph and self.runtime.graph_engine:
            try:
                graph_results = graph_relevance_search(
                    self.runtime.graph_engine,
                    options.query,
                    limit=overfetch,
                )
                if graph_results:
                    ranked_lists["graph"] = graph_results
                    engines_used.append("graph")
            except Exception as exc:
                logger.warning("Graph relevance search failed: %s", exc)

        return ranked_lists, engines_used

    def _fuse_results(
        self,
        ranked_lists: dict[str, list[dict[str, Any]]],
        *,
        route: str,
    ) -> list[dict[str, Any]]:
        if len(ranked_lists) > 1:
            fusion = ReciprocalRankFusion(
                weights=self._fusion_weights(route)
            )
            return fusion.fuse(ranked_lists)

        if ranked_lists:
            return list(ranked_lists.values())[0]

        return []

    def _fusion_weights(self, route: str) -> dict[str, float]:
        weights = {
            "vector": self.settings.fusion_weight_vector,
            "bm25": self.settings.fusion_weight_bm25,
            "graph": self.settings.fusion_weight_graph,
        }
        if route == "natural":
            weights.update({"vector": 0.35, "bm25": 0.65, "graph": 0.0})
        elif route == "symbol":
            weights.update({"vector": 0.0, "bm25": 0.7, "graph": 0.3})
        total = sum(weights.values()) or 1.0
        return {engine: weight / total for engine, weight in weights.items()}

    @staticmethod
    def _annotate_match_sources(results: list[dict[str, Any]]) -> None:
        for result in results:
            sources = result.pop("_fusion_sources", None) or []
            if sources:
                result["match"] = "+".join(
                    sorted(set("semantic" if source == "vector" else source for source in sources))
                )

    def _rerank_results(
        self,
        options: SearchExecutionOptions,
        results: list[dict[str, Any]],
        limit: int,
    ) -> list[dict[str, Any]]:
        if not options.rerank or not results or len(results) <= min(limit, 2):
            return results[:limit]

        try:
            reranker = self._get_reranker()
            candidate_limit = max(
                limit,
                int(getattr(self.settings, "search_rerank_max_candidates", 50)),
            )
            reranked = reranker.rerank(
                options.query,
                results[:candidate_limit],
                limit=limit,
                max_passage_chars=self._rerank_passage_chars(options),
            )
            reranked_results = [dict(result) for result in reranked if isinstance(result, dict)]
            return reranked_results or results[:limit]
        except Exception as exc:
            logger.warning("Reranking failed, using unranked results: %s", exc)
            return results[:limit]

    def _get_reranker(self) -> FlashReranker:
        if not hasattr(self.state, "_reranker") or self.state._reranker is None:
            max_passage_chars = int(getattr(self.settings, "search_rerank_max_passage_chars", 0))
            self.state._reranker = FlashReranker(
                model_name=self.settings.reranker_model,
                max_passage_chars=max_passage_chars if max_passage_chars > 0 else None,
            )
        return self.state._reranker

    def _rerank_passage_chars(self, options: SearchExecutionOptions) -> int | None:
        default_chars = int(getattr(self.settings, "search_rerank_max_passage_chars", 0))
        if options.mode == "vector":
            return default_chars if default_chars > 0 else None

        non_vector_chars = int(getattr(self.settings, "search_rerank_non_vector_passage_chars", 0))
        if non_vector_chars > 0:
            return non_vector_chars
        return default_chars if default_chars > 0 else None

    def _filter_relevance(self, results: list[dict[str, Any]]) -> list[dict[str, Any]]:
        if len(results) <= 1:
            return results

        top_score = results[0].get("score", 1.0)
        if top_score <= 0:
            return results

        threshold = top_score * self.settings.relevance_threshold
        return [result for result in results if result.get("score", 0.0) >= threshold]

    @staticmethod
    def _apply_same_file_diversity(results: list[dict[str, Any]]) -> list[dict[str, Any]]:
        if len(results) <= 1:
            return results

        seen_files: set[str] = set()
        for result in results:
            file_path = result.get("filepath", result.get("file", ""))
            if file_path in seen_files:
                result["score"] = round(result.get("score", 0.0) * 0.7, 3)
            else:
                seen_files.add(file_path)

        results.sort(key=lambda item: item.get("score", 0.0), reverse=True)
        return results

    @staticmethod
    def _dedupe_overlapping_results(results: list[dict[str, Any]]) -> list[dict[str, Any]]:
        if len(results) <= 1:
            return results

        deduped: list[dict[str, Any]] = []
        seen_spans: dict[str, list[tuple[int, int]]] = {}
        for result in results:
            file_path = result.get("filepath", "")
            start = result.get("line_start", 0)
            end = result.get("line_end", start)

            file_spans = seen_spans.setdefault(file_path, [])
            overlaps = any(
                span_start <= end and span_end >= start for span_start, span_end in file_spans
            )
            if overlaps:
                continue

            file_spans.append((start, end))
            deduped.append(result)

        return deduped

    def _apply_bm25_fallback(
        self,
        options: SearchExecutionOptions,
        filters: dict[str, str],
        results: list[dict[str, Any]],
        limit: int,
        engines_used: list[str],
    ) -> list[dict[str, Any]]:
        if (
            options.mode not in ("hybrid", "vector")
            or len(results) >= 3
            or not self.runtime.bm25_engine
        ):
            return results

        try:
            fallback_results = self.runtime.bm25_engine.search(
                options.query,
                limit=limit,
                **filters,
            )
        except Exception as exc:
            logger.debug("BM25 fallback failed: %s", exc)
            return results

        if not fallback_results:
            return results

        seen_keys = {
            (
                result.get("filepath", result.get("file", "")),
                result.get("line_start", result.get("line", 0)),
            )
            for result in results
        }
        for fallback in fallback_results:
            if len(results) >= limit:
                break
            key = (fallback.get("filepath", ""), fallback.get("line_start", 0))
            if key not in seen_keys:
                results.append(fallback)
                seen_keys.add(key)

        if "bm25_fallback" not in engines_used:
            engines_used.append("bm25_fallback")

        return results

    def _apply_live_grep(
        self,
        options: SearchExecutionOptions,
        results: list[dict[str, Any]],
        limit: int,
        engines_used: list[str],
    ) -> list[dict[str, Any]]:
        if not self.runtime.codebase_path:
            return results

        should_run = options.live_grep or (
            options.mode == "hybrid"
            and len(results) < limit
            and not options.language
            and self._should_auto_live_grep(options.query)
        )
        if not should_run:
            return results

        try:
            live_engine = LiveGrepEngine(str(self.runtime.codebase_path))
            live_results = live_engine.search(options.query, limit=limit)
        except Exception as exc:
            logger.warning("Live grep failed: %s", exc)
            return results

        if not live_results:
            return results

        seen = {(result.get("absolute_path"), result.get("line_start")) for result in results}
        for live_result in live_results:
            key = (live_result.get("absolute_path"), live_result.get("line_start"))
            if key not in seen:
                results.append(live_result)
                seen.add(key)

        if "live_grep" not in engines_used:
            engines_used.append("live_grep")

        return results

    @staticmethod
    def _should_auto_live_grep(query: str) -> bool:
        stripped = query.strip()
        if not stripped:
            return False

        # Multi-word natural-language queries are better served by the indexed
        # engines. Falling back to repo-wide grep for those queries can turn a
        # partial search result into a minutes-long scan.
        return len(stripped.split()) == 1

    def _compact_results(self, results: list[dict[str, Any]], query: str) -> list[dict[str, Any]]:
        return self._compactor.compact(results, query)

    @staticmethod
    def _apply_bookend_ordering(results: list[dict[str, Any]]) -> list[dict[str, Any]]:
        if len(results) < 4:
            return results

        first = results[0]
        last = results[1]
        middle = results[2:]
        return [first] + middle + [last]

    @staticmethod
    def _calculate_confidence(results: list[dict[str, Any]]) -> str:
        if not results:
            return "low"

        top = results[0].get("score", 0.0)
        second = results[1].get("score", 0.0) if len(results) > 1 else 0.0
        gap = top - second if second else top

        if top >= 0.7 and gap >= 0.2 and len(results) >= 2:
            return "high"
        if top >= 0.5 or len(results) >= 3:
            return "medium"
        return "low"

    def _apply_adaptive_result_count(
        self,
        options: SearchExecutionOptions,
        results: list[dict[str, Any]],
        confidence: str,
    ) -> tuple[list[dict[str, Any]], bool]:
        if (
            not getattr(self.settings, "search_adaptive_result_count_enabled", True)
            or not results
            or len(results) <= 1
        ):
            return results, False

        adaptive_limit = options.limit
        if confidence == "high":
            adaptive_limit = min(
                adaptive_limit,
                max(1, int(getattr(self.settings, "search_adaptive_high_confidence_limit", 3))),
            )
        elif confidence == "medium":
            adaptive_limit = min(
                adaptive_limit,
                max(2, int(getattr(self.settings, "search_adaptive_medium_confidence_limit", 6))),
            )

        if adaptive_limit >= len(results):
            return results, False

        return results[:adaptive_limit], True
