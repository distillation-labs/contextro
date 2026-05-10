"""Metric fan-out helpers for partner lifecycle workflows."""

from __future__ import annotations

PARTNER_METRIC_PREFIX = "partners.lifecycle"


def emit_partner_metric(metric_name: str, actor_id: str) -> str:
    """Emit a prefixed metric entry for downstream reporting."""
    return f"{PARTNER_METRIC_PREFIX}.{metric_name}:{actor_id}"
