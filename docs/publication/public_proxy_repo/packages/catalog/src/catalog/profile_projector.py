"""Projection builder for partner profile cards."""

from __future__ import annotations

from shared.serializers import serialize_projection_row


class PartnerProfileProjector:
    """Project profile rows into dashboard-friendly partner cards."""

    def project(self, alias: str, profile: dict[str, object]) -> dict[str, object]:
        base_row = serialize_projection_row(alias, profile)
        return {"alias": alias, "row": base_row, "layout": "partner_card"}
