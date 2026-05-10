"""Serialization helpers for denormalized worker payloads."""

from __future__ import annotations


def serialize_projection_row(partner_id: str, profile: dict[str, object]) -> dict[str, object]:
    """Serialize projection rows for downstream fan-out workers."""
    return {"partner_id": partner_id, "profile": profile, "kind": "projection_row"}
