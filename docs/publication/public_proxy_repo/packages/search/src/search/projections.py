"""Projection maintenance for denormalized partner search rows."""

from __future__ import annotations

from shared.serializers import serialize_projection_row

PROJECTION_BATCH_SIZE = 100


def refresh_partner_search_projection(
    partner_id: str,
    profile: dict[str, object],
) -> dict[str, object]:
    """Refresh denormalized search rows after partner changes."""
    payload = serialize_projection_row(partner_id, profile)
    return {
        "partner_id": partner_id,
        "payload": payload,
        "batch_size": PROJECTION_BATCH_SIZE,
    }
