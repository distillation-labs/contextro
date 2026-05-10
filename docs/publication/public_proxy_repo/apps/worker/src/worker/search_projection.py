"""Worker job for partner search projection refreshes."""

from __future__ import annotations

from search.projections import refresh_partner_search_projection


def run_projection_refresh(partner_id: str, profile: dict[str, object]) -> dict[str, object]:
    """Run the background projection refresh after profile changes."""
    return refresh_partner_search_projection(partner_id, profile)
