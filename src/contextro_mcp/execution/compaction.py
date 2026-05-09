"""Token-aware result compaction helpers for search execution.

This module keeps snippet compression and payload compaction logic isolated from
the core retrieval flow so it can be tuned independently and re-used by future
execution engines.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any

TOKEN_RE = re.compile(r"[A-Za-z0-9]+")
CAMEL_BOUNDARY_RE = re.compile(r"([a-z0-9])([A-Z])")


@dataclass(frozen=True, slots=True)
class CodeCompressionBudgets:
    """Configurable character budgets for search-result code snippets."""

    top_chars: int = 200
    second_chars: int = 120
    tail_chars: int = 80
    min_focus_chars: int = 60
    query_window_radius: int = 2
    query_aware: bool = True

    @classmethod
    def from_settings(cls, settings: Any) -> "CodeCompressionBudgets":
        """Build budgets from Settings while clamping invalid values."""

        def _positive_int(value: Any, fallback: int) -> int:
            try:
                parsed = int(value)
            except (TypeError, ValueError):
                return fallback
            return parsed if parsed > 0 else fallback

        return cls(
            top_chars=_positive_int(getattr(settings, "search_code_budget_top_chars", 320), 320),
            second_chars=_positive_int(
                getattr(settings, "search_code_budget_second_chars", 220), 220
            ),
            tail_chars=_positive_int(getattr(settings, "search_code_budget_tail_chars", 80), 80),
            min_focus_chars=_positive_int(
                getattr(settings, "search_code_focus_min_chars", 60), 60
            ),
            query_window_radius=max(
                1,
                _positive_int(getattr(settings, "search_query_window_radius", 2), 2),
            ),
            query_aware=bool(getattr(settings, "search_query_aware_compression", True)),
        )


class SearchResultCompactor:
    """Compacts raw engine results into token-efficient payloads."""

    def __init__(
        self,
        *,
        codebase_paths: tuple[Path, ...],
        budgets: CodeCompressionBudgets,
    ):
        self._codebase_paths = codebase_paths
        self._budgets = budgets

    def compact(self, results: list[dict[str, Any]], query: str) -> list[dict[str, Any]]:
        """Convert engine result rows into compact transport payloads."""
        # Filter file-overview chunks when there are enough specific results.
        # File overviews pollute function-level searches with file-level summaries.
        # Keep them only if they're the only results or the query looks file-level.
        non_overview = [r for r in results if r.get("symbol_type") != "file_overview"]
        if len(non_overview) >= 2:
            results = non_overview

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
            entry.pop("match", None)          # match type is metadata agents don't act on
            entry.pop("line_end", None)       # line_end is always derivable, saves ~8 chars/result
            if not entry.get("language"):
                entry.pop("language", None)
            # Only top result gets confidence — others are always lower quality
            if index > 0:
                entry.pop("confidence", None)

            if "score" in entry:
                entry["score"] = round(entry["score"], 3)
            # Only top result gets score — agents use rank, not exact score
            if index > 0:
                entry.pop("score", None)

            symbol_name = entry.get("symbol_name", "")
            if symbol_name.startswith("[rel] "):
                entry["symbol_name"] = symbol_name[6:]
            elif symbol_name.startswith("[file] "):
                entry["symbol_name"] = symbol_name[7:]

            if "filepath" in entry:
                entry["filepath"] = self._relative_filepath(entry["filepath"])

            if "symbol_name" in entry:
                entry["n"] = entry.pop("symbol_name")
            if "filepath" in entry:
                entry["f"] = entry.pop("filepath")
            if "line_start" in entry:
                entry["l"] = entry.pop("line_start")
            if "text" in entry:
                code = entry.pop("text")
                budget = self._body_char_budget(index)
                if budget > 0:
                    entry["c"] = self._compress_code(code, index, query)
            entry.pop("absolute_path", None)
            compacted.append(entry)

        # Drop language field when all results share the same language —
        # move it to the response level instead (caller adds it if needed).
        # This saves ~5 tokens × N results per search.
        if compacted:
            langs = {r.get("language") for r in compacted if r.get("language")}
            if len(langs) == 1:
                for r in compacted:
                    r.pop("language", None)

        return compacted

    def _relative_filepath(self, filepath: str) -> str:
        for root in self._codebase_paths:
            try:
                return str(Path(filepath).relative_to(root))
            except ValueError:
                continue
        return filepath

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
                or line.startswith("method:")
                or line.startswith("class:")
                or line.startswith("module:")
                or line.startswith("# Relationship context:")
                or line.startswith("# File overview:")
                or line == ""
            ):
                continue

            in_header = False
            if line.startswith("Calls: "):
                # Truncate to first 5 callees — full list is too verbose
                callees = line[7:].split(", ")
                call_line = "Calls: " + ", ".join(callees[:5]) + ("…" if len(callees) > 5 else "")
            elif line.startswith("Imports: "):
                continue
            elif line.startswith("  → "):
                signature_lines.append(line)
            elif (
                not line.startswith(" ")
                and not line.startswith("\t")
                and not any(
                    c in line
                    for c in ("(", ":", "=", "{", "[", "def ", "class ", "return ", "if ", "for ")
                )
                and len(line) < 120
                and body_lines  # skip plain-text lines only after we have some body
            ):
                # Skip plain docstring text lines (no code structure)
                continue
            else:
                body_lines.append(line)

        max_body = self._body_char_budget(index)
        fallback_body = self._truncate_body(body_lines, max_body)
        body = fallback_body
        if self._budgets.query_aware:
            focal_body = self._query_focal_body(body_lines, query, max_body)
            query_tokens = self._query_tokens(query)
            fallback_has_match = self._contains_query_match(fallback_body, query_tokens)
            if focal_body and (not fallback_has_match or len(focal_body) <= len(fallback_body)):
                body = focal_body

        parts: list[str] = []
        if body:
            parts.append(body)
        if signature_lines:
            # Compress "→ callee" lines to a single compact line
            callees = [line.strip().lstrip("→ ").strip() for line in signature_lines]
            parts.append("→ " + ", ".join(callees[:5]) + ("…" if len(callees) > 5 else ""))
        if call_line:
            parts.append(call_line)

        return "\n".join(parts) if parts else code[:200]

    def _body_char_budget(self, index: int) -> int:
        if index <= 0:
            return self._budgets.top_chars
        # Only top result gets code — name+file+line is enough for navigation
        return 0

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
            radius=self._budgets.query_window_radius,
        )
        return self._trim_window_lines(
            body_lines[best_start:best_end],
            line_scores[best_start:best_end],
            focus_index,
            max_body,
        )

    @staticmethod
    def _best_scoring_window(line_scores: list[float], radius: int) -> tuple[int, int, int]:
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

    def _trim_window_lines(
        self,
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
            if (
                current_len + added_len > max_body
                and current_len >= min(max_body, self._budgets.min_focus_chars)
            ):
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

        line_tokens = SearchResultCompactor._query_tokens(stripped)
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

        text_tokens = SearchResultCompactor._query_tokens(text)
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
