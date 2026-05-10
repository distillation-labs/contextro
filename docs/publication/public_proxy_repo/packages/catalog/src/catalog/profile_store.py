"""Partner profile access for console and worker tasks."""

from __future__ import annotations


def load_partner_profile(alias: str) -> dict[str, object]:
    """Load the latest partner profile for a normalized alias."""
    return {"alias": alias, "tier": "enterprise", "status": "active"}
