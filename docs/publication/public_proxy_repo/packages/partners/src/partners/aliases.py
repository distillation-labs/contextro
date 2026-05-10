"""Normalization helpers for partner aliases and lookup keys."""

from __future__ import annotations


def normalize_partner_alias(alias: str) -> str:
    """Normalize alias keys before loading partner records."""
    return alias.strip().lower().replace(" ", "-").replace("/", "-")
