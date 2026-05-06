"""Shared search execution engine for Contextia."""

from __future__ import annotations

import json
import logging
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from contextia_mcp.engines.fusion import ReciprocalRankFusion, graph_relevance_search
from contextia_mcp.engines.live_grep import LiveGrepEngine
from contextia_mcp.engines.reranker import FlashReranker
from contextia_mcp.execution.runtime import SearchRuntime

logger = logging.getLogger(__name__)
TOKEN_RE = re.compile(r"[A-Za-z0-9]+")
CAMEL_BOUNDARY_RE = re.compile(r"([a-z0-9])([A-Z])")


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
    """Provider-agnostic execution engine for Contextia search flows."""

    def __init__(self, runtime: SearchRuntime):
        self.runtime = runtime
        self.state = runtime.state
        self.settings = runtime.settings

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
        ranked_lists, engines_used = self._collect_ranked_lists(
            options,
            limit=limit,
            filters=filters,
            query_embedding=query_embedding,
        )

        results = self._fuse_results(ranked_lists)
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
        full_results = [dict(result) for result in results]
        response = self._build_response(options.query, results, confidence)
        response, budget_applied = self._apply_context_budget(response, options.context_budget)
        response = self._maybe_sandbox_response(
            query=options.query,
            response=response,
            full_results=full_results,
            budget_applied=budget_applied,
        )

        self.runtime.query_cache.put(
            options.query,
            response,
            query_embedding=query_embedding,
            namespace=cache_namespace,
        )
        self.runtime.session_tracker.track_search(options.query, response["total"])
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
    ) -> tuple[dict[str, list[dict[str, Any]]], list[str]]:
        ranked_lists: dict[str, list[dict[str, Any]]] = {}
        engines_used: list[str] = []
        overfetch = limit * 2

        if options.mode in ("hybrid", "vector") and self.runtime.vector_engine:
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

        if options.mode in ("hybrid", "bm25") and self.runtime.bm25_engine:
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

        if options.mode == "hybrid" and self.runtime.graph_engine:
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

    def _fuse_results(self, ranked_lists: dict[str, list[dict[str, Any]]]) -> list[dict[str, Any]]:
        if len(ranked_lists) > 1:
            fusion = ReciprocalRankFusion(
                weights={
                    "vector": self.settings.fusion_weight_vector,
                    "bm25": self.settings.fusion_weight_bm25,
                    "graph": self.settings.fusion_weight_graph,
                }
            )
            return fusion.fuse(ranked_lists)

        if ranked_lists:
            return list(ranked_lists.values())[0]

        return []

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
        if not options.rerank or not results:
            return results[:limit]

        try:
            reranker = self._get_reranker()
            reranked = reranker.rerank(options.query, results[:50], limit=limit)
            reranked_results = [
                dict(result) for result in reranked if isinstance(result, dict)
            ]
            return reranked_results or results[:limit]
        except Exception as exc:
            logger.warning("Reranking failed, using unranked results: %s", exc)
            return results[:limit]

    def _get_reranker(self) -> FlashReranker:
        if not hasattr(self.state, "_reranker") or self.state._reranker is None:
            self.state._reranker = FlashReranker(model_name=self.settings.reranker_model)
        return self.state._reranker

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
                span_start <= end and span_end >= start
                for span_start, span_end in file_spans
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
            options.mode == "hybrid" and len(results) < limit and not options.language
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

    def _compact_results(
        self, results: list[dict[str, Any]], query: str
    ) -> list[dict[str, Any]]:
        roots = self.runtime.codebase_paths
        compacted: list[dict[str, Any]] = []

        for index, result in enumerate(results):
            entry = dict(result)
            entry.pop("vector", None)
            entry.pop("docstring", None)
            entry.pop("id", None)
            entry.pop("_fusion_sources", None)
            entry.pop("signature", None)
            entry.pop("parent", None)
            entry.pop("rrf_score", None)
            entry.pop("rerank_score", None)
            entry.pop("symbol_type", None)
            if not entry.get("language"):
                entry.pop("language", None)

            if entry.get("line_end") and entry.get("line_start"):
                if entry["line_end"] - entry["line_start"] <= 1:
                    entry.pop("line_end", None)

            if "score" in entry:
                entry["score"] = round(entry["score"], 3)

            symbol_name = entry.get("symbol_name", "")
            if symbol_name.startswith("[rel] "):
                entry["symbol_name"] = symbol_name[6:]
            elif symbol_name.startswith("[file] "):
                entry["symbol_name"] = symbol_name[7:]

            file_path = entry.get("filepath", "")
            for root in roots:
                try:
                    entry["filepath"] = str(Path(file_path).relative_to(root))
                    break
                except ValueError:
                    continue

            if "symbol_name" in entry:
                entry["name"] = entry.pop("symbol_name")
            if "filepath" in entry:
                entry["file"] = entry.pop("filepath")
            if "line_start" in entry:
                entry["line"] = entry.pop("line_start")
            if "text" in entry:
                entry["code"] = self._compress_code(entry.pop("text"), index, query)
            entry.pop("absolute_path", None)
            compacted.append(entry)

        return compacted

    def _compress_code(self, code: str, index: int, query: str) -> str:
        lines = code.split("\n")
        signature_lines: list[str] = []
        call_line = ""
        body_lines: list[str] = []
        in_header = True

        for line in lines:
            if in_header and (
                line.startswith("# ")
                or line.startswith("function:")
                or line.startswith("class:")
                or line.startswith("module:")
                or line.startswith("# Relationship context:")
                or line.startswith("# File overview:")
                or line == ""
            ):
                continue

            in_header = False
            if line.startswith("Calls: "):
                call_line = line
            elif line.startswith("Imports: "):
                continue
            elif line.startswith("  → "):
                signature_lines.append(line)
            else:
                body_lines.append(line)

        max_body = self._body_char_budget(index)
        fallback_body = self._truncate_body(body_lines, max_body)
        body = fallback_body
        if self.settings.search_query_aware_compression:
            focal_body = self._query_focal_body(body_lines, query, max_body)
            query_tokens = self._query_tokens(query)
            fallback_has_match = self._contains_query_match(fallback_body, query_tokens)
            if focal_body and (
                not fallback_has_match or len(focal_body) <= len(fallback_body)
            ):
                body = focal_body

        parts: list[str] = []
        if body:
            parts.append(body)
        if signature_lines:
            parts.append("\n".join(signature_lines))
        if call_line:
            parts.append(call_line)

        return "\n".join(parts) if parts else code[:200]

    @staticmethod
    def _body_char_budget(index: int) -> int:
        if index <= 0:
            return 320
        if index == 1:
            return 220
        return 80

    @staticmethod
    def _truncate_body(body_lines: list[str], max_body: int) -> str:
        body = "\n".join(body_lines).strip()
        if len(body) > max_body:
            return body[:max_body] + "…"
        return body

    def _query_focal_body(self, body_lines: list[str], query: str, max_body: int) -> str:
        query_tokens = self._query_tokens(query)
        if not body_lines or not query_tokens:
            return ""

        line_scores = [self._line_match_score(line, query_tokens) for line in body_lines]
        if max(line_scores, default=0.0) <= 0:
            return ""

        best_start, best_end, focus_index = self._best_scoring_window(
            line_scores,
            radius=max(1, self.settings.search_query_window_radius),
        )
        return self._trim_window_lines(
            body_lines[best_start:best_end],
            line_scores[best_start:best_end],
            focus_index,
            max_body,
        )

    @staticmethod
    def _best_scoring_window(
        line_scores: list[float], radius: int
    ) -> tuple[int, int, int]:
        window_size = max(1, (radius * 2) + 1)
        best_start = 0
        best_end = min(len(line_scores), window_size)
        best_score = -1.0
        best_focus = 0
        best_focus_score = -1.0

        for start in range(max(1, len(line_scores) - window_size + 1)):
            end = min(len(line_scores), start + window_size)
            window_scores = line_scores[start:end]
            score = sum(window_scores)
            focus = max(range(len(window_scores)), key=window_scores.__getitem__)
            focus_score = window_scores[focus]
            if score > best_score or (score == best_score and focus_score > best_focus_score):
                best_score = score
                best_start = start
                best_end = end
                best_focus = focus
                best_focus_score = focus_score

        return best_start, best_end, best_focus

    @staticmethod
    def _trim_window_lines(
        lines: list[str],
        scores: list[float],
        focus_index: int,
        max_body: int,
    ) -> str:
        if not lines:
            return ""

        left = focus_index
        right = focus_index + 1
        selected = [lines[focus_index].rstrip()]
        current_len = len(selected[0])

        while left > 0 or right < len(lines):
            left_score = scores[left - 1] if left > 0 else -1.0
            right_score = scores[right] if right < len(lines) else -1.0
            choose_left = left > 0 and (right >= len(lines) or left_score >= right_score)
            next_index = left - 1 if choose_left else right
            next_line = lines[next_index].rstrip()
            added_len = len(next_line) + 1
            if current_len + added_len > max_body and current_len >= min(max_body, 60):
                break

            if choose_left:
                selected.insert(0, next_line)
                left -= 1
            else:
                selected.append(next_line)
                right += 1
            current_len += added_len

        snippet = "\n".join(line for line in selected if line).strip()
        if len(snippet) > max_body:
            return snippet[:max_body] + "…"
        return snippet

    @staticmethod
    def _line_match_score(line: str, query_tokens: set[str]) -> float:
        stripped = line.strip()
        if not stripped:
            return 0.0

        line_tokens = SearchExecutionEngine._query_tokens(stripped)
        overlap = len(query_tokens & line_tokens)
        lowered = stripped.lower()
        substring_hits = sum(1 for token in query_tokens if token in lowered)
        if overlap == 0 and substring_hits == 0:
            return 0.0

        structural_bonus = 0.25 if lowered.startswith(
            ("def ", "class ", "return ", "if ", "elif ", "raise ", "await ", "for ", "while ")
        ) else 0.0
        return (overlap * 2.0) + (substring_hits * 0.5) + structural_bonus

    @staticmethod
    def _contains_query_match(text: str, query_tokens: set[str]) -> bool:
        if not text or not query_tokens:
            return False

        text_tokens = SearchExecutionEngine._query_tokens(text)
        if query_tokens & text_tokens:
            return True

        lowered = text.lower()
        return any(token in lowered for token in query_tokens)

    @staticmethod
    def _query_tokens(text: str) -> set[str]:
        normalized = CAMEL_BOUNDARY_RE.sub(r"\1 \2", text.replace("_", " "))
        return {
            token.lower()
            for token in TOKEN_RE.findall(normalized)
            if len(token) > 1 and not token.isdigit()
        }

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

    @staticmethod
    def _build_response(
        query: str,
        results: list[dict[str, Any]],
        confidence: str,
    ) -> dict[str, Any]:
        response = {
            "query": query,
            "total": len(results),
            "confidence": confidence,
            "results": results,
        }
        response["tokens"] = len(json.dumps(response, default=str)) // 4
        return response

    @staticmethod
    def _apply_context_budget(
        response: dict[str, Any], context_budget: int
    ) -> tuple[dict[str, Any], bool]:
        if context_budget <= 0 or not response["results"]:
            return response, False

        budget_chars = context_budget * 4
        kept: list[dict[str, Any]] = []
        used = 0
        for result in response["results"]:
            result_size = len(json.dumps(result, default=str))
            if used + result_size > budget_chars and kept:
                break
            kept.append(result)
            used += result_size

        if not kept:
            return response, False

        updated = dict(response)
        updated["results"] = kept
        updated["total"] = len(kept)
        updated["tokens"] = len(json.dumps(updated, default=str)) // 4
        updated["budget_applied"] = True
        return updated, True

    def _maybe_sandbox_response(
        self,
        query: str,
        response: dict[str, Any],
        full_results: list[dict[str, Any]],
        budget_applied: bool,
    ) -> dict[str, Any]:
        if not full_results:
            return response

        threshold = self.settings.search_sandbox_threshold_tokens
        should_sandbox = budget_applied or response.get("tokens", 0) > threshold
        if not should_sandbox:
            return response

        payload = json.dumps(
            {
                "query": query,
                "confidence": response.get("confidence", "low"),
                "results": full_results,
            },
            indent=2,
            default=str,
        )
        sandbox_ref = self.runtime.output_sandbox.store(
            payload,
            metadata={"query": query, "total": len(full_results)},
        )

        preview_results = (
            response["results"]
            if budget_applied
            else self._preview_results(full_results)
        )
        preview = dict(response)
        preview["results"] = preview_results
        preview["total"] = len(preview_results)
        preview["full_total"] = len(full_results)
        preview["sandbox_ref"] = sandbox_ref
        preview["sandboxed"] = True
        preview["hint"] = (
            "Call retrieve() with sandbox_ref to inspect the full result set."
            if not budget_applied
            else (
                "Inline results were budget-trimmed. "
                "Call retrieve() with sandbox_ref for the full set."
            )
        )
        preview["tokens"] = len(json.dumps(preview, default=str)) // 4
        return preview

    def _preview_results(self, results: list[dict[str, Any]]) -> list[dict[str, Any]]:
        preview_count = max(1, self.settings.search_preview_results)
        code_chars = max(80, self.settings.search_preview_code_chars)

        preview: list[dict[str, Any]] = []
        for index, result in enumerate(results[:preview_count]):
            entry = dict(result)
            if index > 0 and entry.get("code"):
                code = entry["code"]
                entry["code"] = code[:code_chars] + ("…" if len(code) > code_chars else "")
            preview.append(entry)
        return preview
