"""Search response assembly and sandbox policy helpers."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any

from contextia_mcp.engines.output_sandbox import OutputSandbox


def _estimate_tokens(payload: dict[str, Any]) -> int:
    return len(json.dumps(payload, default=str)) // 4


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
                entry["code"] = code[:code_chars] + ("…" if len(code) > code_chars else "")
            preview.append(entry)

        return preview
