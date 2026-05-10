"""Ranking heuristics for partner search results."""

from __future__ import annotations


def score_partner_hits(text_score: float, freshness_score: float, affinity_score: float) -> float:
    """Compute weighted ranking for partner discovery hits."""
    return round((text_score * 0.5) + (freshness_score * 0.2) + (affinity_score * 0.3), 4)
