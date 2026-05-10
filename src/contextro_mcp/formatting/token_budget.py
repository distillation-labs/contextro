"""Token budget management for response size control."""

from __future__ import annotations

from contextro_mcp.token_counting import count_text_tokens, truncate_text_to_tokens


class TokenBudget:
    """Estimate and enforce token budgets using a real tokenizer."""

    BUDGETS = {
        "summary": 500,
        "detailed": 2000,
        "full": 8000,
    }

    def __init__(self, verbosity: str = "detailed"):
        if verbosity not in self.BUDGETS:
            raise ValueError(
                f"verbosity must be one of {list(self.BUDGETS.keys())}, got '{verbosity}'"
            )
        self.verbosity = verbosity
        self.budget_tokens = self.BUDGETS[verbosity]

    def estimate_tokens(self, text: str) -> int:
        """Estimate token count for a string."""
        return count_text_tokens(text)

    def fits(self, text: str) -> bool:
        """Check if text fits within the budget."""
        return self.estimate_tokens(text) <= self.budget_tokens

    def remaining(self, used_tokens: int) -> int:
        """Return remaining token budget after used_tokens."""
        return max(0, self.budget_tokens - used_tokens)

    def truncate(self, text: str, reserve: int = 0) -> str:
        """Truncate text to fit within the token budget minus reserve."""
        limit = self.budget_tokens - reserve
        if limit <= 0:
            return ""
        if self.estimate_tokens(text) <= limit:
            return text

        ellipsis = "..."
        ellipsis_tokens = count_text_tokens(ellipsis)
        truncated = truncate_text_to_tokens(text, max(1, limit - ellipsis_tokens))
        last_space = truncated.rfind(" ")
        if last_space > len(truncated) // 2:
            truncated = truncated[:last_space]

        return truncated.rstrip() + ellipsis
