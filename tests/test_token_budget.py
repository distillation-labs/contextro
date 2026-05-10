"""Tests for token budget management."""

import pytest

from contextro_mcp.formatting.token_budget import TokenBudget
from contextro_mcp.token_counting import count_text_tokens


class TestEstimation:
    def test_estimate_tokens(self):
        budget = TokenBudget("detailed")
        assert budget.estimate_tokens("abcd") == 1
        assert budget.estimate_tokens("abcdefgh") >= 1
        assert budget.estimate_tokens("abcdefgh") == count_text_tokens("abcdefgh")

    def test_estimate_empty(self):
        budget = TokenBudget("detailed")
        assert budget.estimate_tokens("") == 0


class TestFits:
    def test_fits_under_budget(self):
        budget = TokenBudget("summary")
        assert budget.fits("x" * 1999) is True

    def test_fits_at_budget(self):
        budget = TokenBudget("summary")
        text = "x" * 4000
        assert budget.estimate_tokens(text) == budget.budget_tokens
        assert budget.fits(text) is True

    def test_does_not_fit_over_budget(self):
        budget = TokenBudget("summary")
        assert budget.fits("x" * 4001) is False


class TestTruncation:
    def test_truncate_short_text(self):
        budget = TokenBudget("summary")
        text = "short text"
        assert budget.truncate(text) == text

    def test_truncate_long_text(self):
        budget = TokenBudget("summary")
        text = "word " * 500
        result = budget.truncate(text)
        assert budget.estimate_tokens(result) <= budget.budget_tokens
        assert result.endswith("...")

    def test_truncate_with_reserve(self):
        budget = TokenBudget("summary")
        text = "word " * 500
        result = budget.truncate(text, reserve=450)
        assert budget.estimate_tokens(result) <= 50
        assert result.endswith("...")

    def test_truncate_zero_budget(self):
        budget = TokenBudget("summary")
        result = budget.truncate("text", reserve=3000)
        assert result == ""


class TestVerbosityLevels:
    def test_summary_budget(self):
        budget = TokenBudget("summary")
        assert budget.budget_tokens == 500

    def test_detailed_budget(self):
        budget = TokenBudget("detailed")
        assert budget.budget_tokens == 2000

    def test_full_budget(self):
        budget = TokenBudget("full")
        assert budget.budget_tokens == 8000

    def test_invalid_verbosity(self):
        with pytest.raises(ValueError, match="verbosity must be one of"):
            TokenBudget("invalid")

    def test_budgets_ordered(self):
        s = TokenBudget("summary")
        d = TokenBudget("detailed")
        f = TokenBudget("full")
        assert s.budget_tokens < d.budget_tokens < f.budget_tokens


class TestRemaining:
    def test_remaining_positive(self):
        budget = TokenBudget("summary")
        assert budget.remaining(100) == 400

    def test_remaining_zero(self):
        budget = TokenBudget("summary")
        assert budget.remaining(500) == 0

    def test_remaining_negative_clamps(self):
        budget = TokenBudget("summary")
        assert budget.remaining(600) == 0
