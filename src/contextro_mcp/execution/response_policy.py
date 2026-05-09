"""Response assembly and sandbox policy helpers.

Provides progressive disclosure for all tools: compact summaries inline,
full content available on demand via retrieve(). Inspired by Cursor's
"dynamic context discovery" pattern (46.9% token reduction in A/B test)
and Anthropic's principle of "smallest possible set of high-signal tokens".
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any

from contextro_mcp.engines.output_sandbox import OutputSandbox


def _serialize_payload(payload: dict[str, Any] | list | str, *, pretty: bool = False) -> str:
    if isinstance(payload, str):
        return payload
    if pretty:
        return json.dumps(payload, indent=2, default=str)
    return json.dumps(payload, default=str, separators=(",", ":"))


def _estimate_tokens(payload: dict[str, Any] | list | str) -> int:
    return len(_serialize_payload(payload)) // 4


_PREVIEW_STRING_CHARS = 280
_PREVIEW_NESTED_DICT_ITEMS = 6
_PREVIEW_NESTED_LIST_ITEMS = 3


def _truncate_string(
    value: str, *, max_chars: int = _PREVIEW_STRING_CHARS
) -> tuple[str, int | None]:
    if len(value) <= max_chars:
        return value, None
    return value[:max_chars] + "…", len(value)


# ---------------------------------------------------------------------------
# Universal progressive disclosure for all tools
# ---------------------------------------------------------------------------


class ToolResponsePolicy:
    """Applies progressive disclosure to any tool response.

    If the response exceeds the token threshold, stores the full response
    in the sandbox and returns a compact preview with a sandbox_ref.
    """

    def __init__(
        self,
        *,
        output_sandbox: OutputSandbox,
        threshold_tokens: int = 1200,
    ):
        self._sandbox = output_sandbox
        self._threshold = threshold_tokens

    def apply(
        self,
        response: dict[str, Any],
        *,
        tool_name: str = "",
        preview_keys: list[str] | None = None,
        max_list_items: int = 5,
    ) -> dict[str, Any]:
        """Apply progressive disclosure if response exceeds threshold.

        Args:
            response: The full tool response dict.
            tool_name: Name of the tool (for metadata).
            preview_keys: Keys to always include in preview. If None, uses
                heuristics to pick summary-level keys.
            max_list_items: Max items to keep in list-valued preview fields.

        Returns:
            Either the original response (if small enough) or a compact
            preview with sandbox_ref for full retrieval.
        """
        payload = _serialize_payload(response)
        tokens = len(payload) // 4
        if tokens <= self._threshold:
            return response

        # Store full response in sandbox
        ref_id = self._sandbox.store(
            _serialize_payload(response, pretty=True),
            metadata={"tool": tool_name},
        )

        # Build compact preview
        preview = self._build_preview(response, preview_keys, max_list_items)
        # sandboxed:True omitted — presence of sandbox_ref implies it
        preview["sandbox_ref"] = ref_id
        preview["full_tokens"] = tokens
        return preview

    def _build_preview(
        self,
        response: dict[str, Any],
        preview_keys: list[str] | None,
        max_list_items: int,
    ) -> dict[str, Any]:
        """Extract a compact preview from the full response."""
        preview: dict[str, Any] = {}

        # Always keep scalar/small fields and error fields
        always_keep = {
            "error",
            "symbol",
            "total",
            "total_impacted",
            "confidence",
            "query",
            "max_depth",
            "name",
            "file",
            "line",
            "type",
        }
        if preview_keys:
            always_keep.update(preview_keys)

        for key, value in response.items():
            self._add_preview_field(
                preview,
                key,
                value,
                force_include=key in always_keep,
                max_list_items=max_list_items,
            )

        return preview

    def _add_preview_field(
        self,
        preview: dict[str, Any],
        key: str,
        value: Any,
        *,
        force_include: bool,
        max_list_items: int,
    ) -> None:
        if isinstance(value, list):
            preview[key] = [
                self._preview_value(item, max_list_items=max_list_items, depth=1)
                for item in value[:max_list_items]
            ]
            if len(value) > max_list_items:
                preview[f"{key}_total"] = len(value)
            return

        if isinstance(value, dict):
            if force_include or _estimate_tokens(value) <= 200:
                preview[key] = self._preview_value(value, max_list_items=max_list_items, depth=1)
            else:
                preview[f"{key}_summary"] = f"{len(value)} entries"
            return

        if isinstance(value, str):
            preview[key], total_chars = _truncate_string(value)
            if total_chars is not None:
                preview[f"{key}_chars"] = total_chars
            return

        preview[key] = value

    def _preview_value(self, value: Any, *, max_list_items: int, depth: int) -> Any:
        if isinstance(value, str):
            truncated, _ = _truncate_string(value)
            return truncated

        if isinstance(value, list):
            nested_limit = max(1, min(max_list_items, _PREVIEW_NESTED_LIST_ITEMS))
            return [
                self._preview_value(item, max_list_items=max_list_items, depth=depth + 1)
                for item in value[:nested_limit]
            ]

        if isinstance(value, dict):
            if depth >= 2 and _estimate_tokens(value) > 120:
                return f"{len(value)} entries"

            items = list(value.items())
            limit = len(items) if depth == 0 else min(len(items), _PREVIEW_NESTED_DICT_ITEMS)
            return {
                item_key: self._preview_value(
                    item_value, max_list_items=max_list_items, depth=depth + 1
                )
                for item_key, item_value in items[:limit]
            }

        return value


@dataclass(frozen=True, slots=True)
class SearchResponseSettings:
    """Normalized response-assembly settings for search flows."""

    sandbox_threshold_tokens: int = 1200
    preview_results: int = 4
    preview_code_chars: int = 220

    @classmethod
    def from_settings(cls, settings: Any) -> "SearchResponseSettings":
        def _positive_int(value: Any, fallback: int) -> int:
            try:
                parsed = int(value)
            except (TypeError, ValueError):
                return fallback
            return parsed if parsed > 0 else fallback

        return cls(
            sandbox_threshold_tokens=_positive_int(
                getattr(settings, "search_sandbox_threshold_tokens", 1200),
                1200,
            ),
            preview_results=_positive_int(getattr(settings, "search_preview_results", 4), 4),
            preview_code_chars=_positive_int(
                getattr(settings, "search_preview_code_chars", 220),
                220,
            ),
        )


class SearchResponsePolicy:
    """Applies budgeting and sandbox policy to already-ranked search results."""

    def __init__(self, *, output_sandbox: OutputSandbox, settings: SearchResponseSettings):
        self._output_sandbox = output_sandbox
        self._settings = settings

    def build(
        self,
        *,
        query: str,
        results: list[dict[str, Any]],
        full_results: list[dict[str, Any]] | None = None,
        confidence: str,
        context_budget: int,
        adaptive_applied: bool = False,
        language: str = "",
    ) -> dict[str, Any]:
        all_results = [dict(result) for result in (full_results or results)]
        response = {
            "total": len(results),
            "results": results,
        }
        # confidence omitted when "high" — that's the expected/default case
        if confidence != "high":
            response["confidence"] = confidence
        # Surface language at response level when uniform (compactor strips it per-result)
        # Only include when it adds information (i.e., not already in the search filter)
        if language and language not in ("python", ""):
            response["lang"] = language
        # NOTE: no "tokens" field — self-referential and wastes tokens

        response, budget_applied = self._apply_context_budget(response, context_budget)
        return self._maybe_sandbox_response(
            query=query,
            response=response,
            full_results=all_results,
            budget_applied=budget_applied,
            adaptive_applied=adaptive_applied,
            confidence=confidence,
        )

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
        updated["budget_applied"] = True
        return updated, True

    def _maybe_sandbox_response(
        self,
        *,
        query: str,
        response: dict[str, Any],
        full_results: list[dict[str, Any]],
        budget_applied: bool,
        adaptive_applied: bool,
        confidence: str = "high",
    ) -> dict[str, Any]:
        if not full_results:
            return response

        should_sandbox = adaptive_applied or budget_applied or (
            _estimate_tokens(response) > self._settings.sandbox_threshold_tokens
        )
        if not should_sandbox:
            return response

        payload = _serialize_payload(
            {
                "query": query,
                "confidence": confidence,
                "results": full_results,
            },
            pretty=True,
        )
        sandbox_ref = self._output_sandbox.store(
            payload,
            metadata={"query": query, "total": len(full_results)},
        )

        preview_limit = len(response["results"]) if budget_applied else None
        preview_source = (
            response["results"] if (budget_applied or adaptive_applied) else full_results
        )
        preview_results = self._preview_results(preview_source, limit=preview_limit)
        preview = dict(response)
        preview["results"] = preview_results
        preview["total"] = len(preview_results)
        preview["full_total"] = len(full_results)
        preview["sandbox_ref"] = sandbox_ref
        # sandboxed:True omitted — presence of sandbox_ref implies it
        # adaptive_applied omitted — debugging info, not needed by agents
        if budget_applied:
            preview["budget_applied"] = True
        # Remove self-referential token count
        preview.pop("tokens", None)
        return preview

    def _preview_results(
        self,
        results: list[dict[str, Any]],
        *,
        limit: int | None = None,
    ) -> list[dict[str, Any]]:
        preview: list[dict[str, Any]] = []
        preview_count = max(1, limit or self._settings.preview_results)
        code_chars = max(80, self._settings.preview_code_chars)

        if len(results) <= preview_count:
            preview_source = results
        elif preview_count == 1:
            preview_source = [results[0]]
        else:
            preview_source = [results[0], *results[1 : preview_count - 1], results[-1]]

        for index, result in enumerate(preview_source):
            entry = dict(result)
            if index > 0 and entry.get("code"):
                code = entry["code"]
                # Try AST-aware compression for preview snippets
                if len(code) > code_chars and len(code) > 300:
                    try:
                        from contextro_mcp.execution.ast_compression import compress_snippet

                        lang = entry.get("language", "python")
                        compressed = compress_snippet(code, lang)
                        if len(compressed) < len(code) * 0.8:
                            code = compressed
                    except Exception:
                        pass
                entry["code"] = code[:code_chars] + ("…" if len(code) > code_chars else "")
            preview.append(entry)

        return preview
