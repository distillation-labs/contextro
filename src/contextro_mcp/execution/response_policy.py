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


def _estimate_tokens(payload: dict[str, Any] | list | str) -> int:
    if isinstance(payload, str):
        return len(payload) // 4
    return len(json.dumps(payload, default=str)) // 4


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
        tokens = _estimate_tokens(response)
        if tokens <= self._threshold:
            return response

        # Store full response in sandbox
        payload = json.dumps(response, indent=2, default=str)
        ref_id = self._sandbox.store(payload, metadata={"tool": tool_name})

        # Build compact preview
        preview = self._build_preview(response, preview_keys, max_list_items)
        preview["sandbox_ref"] = ref_id
        preview["full_tokens"] = tokens
        preview["hint"] = "Call retrieve() with sandbox_ref for full details."
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
            if key in always_keep:
                preview[key] = value
            elif isinstance(value, list):
                # Truncate lists to max_list_items
                if len(value) <= max_list_items:
                    preview[key] = value
                else:
                    preview[key] = value[:max_list_items]
                    preview[f"{key}_total"] = len(value)
            elif isinstance(value, dict):
                # Keep dicts only if small
                if _estimate_tokens(value) <= 200:
                    preview[key] = value
                else:
                    preview[f"{key}_summary"] = f"{len(value)} entries"
            else:
                preview[key] = value

        return preview


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
        confidence: str,
        context_budget: int,
    ) -> dict[str, Any]:
        full_results = [dict(result) for result in results]
        response = {
            "query": query,
            "total": len(results),
            "confidence": confidence,
            "results": results,
        }
        response["tokens"] = _estimate_tokens(response)

        response, budget_applied = self._apply_context_budget(response, context_budget)
        return self._maybe_sandbox_response(
            query=query,
            response=response,
            full_results=full_results,
            budget_applied=budget_applied,
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
        updated["tokens"] = _estimate_tokens(updated)
        updated["budget_applied"] = True
        return updated, True

    def _maybe_sandbox_response(
        self,
        *,
        query: str,
        response: dict[str, Any],
        full_results: list[dict[str, Any]],
        budget_applied: bool,
    ) -> dict[str, Any]:
        if not full_results:
            return response

        should_sandbox = budget_applied or (
            response.get("tokens", 0) > self._settings.sandbox_threshold_tokens
        )
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
        sandbox_ref = self._output_sandbox.store(
            payload,
            metadata={"query": query, "total": len(full_results)},
        )

        preview_limit = len(response["results"]) if budget_applied else None
        preview_results = self._preview_results(full_results, limit=preview_limit)
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
        preview["tokens"] = _estimate_tokens(preview)
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
