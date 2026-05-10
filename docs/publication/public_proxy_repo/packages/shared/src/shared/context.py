"""Actor context builders used across console and worker flows."""

from __future__ import annotations


def build_actor_context(actor_id: str) -> dict[str, object]:
    """Build a normalized actor context for partner-facing workflows."""
    return {"actor_id": actor_id, "actor_scope": "partner"}
