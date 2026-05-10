"""Bridge authentication flows to the shared audit stream."""

from __future__ import annotations

from analytics.reporter import emit_partner_metric


def emit_auth_audit_event(event_name: str, actor_id: str) -> dict[str, str]:
    """Emit a partner-scoped audit event and companion metric."""
    emit_partner_metric("auth.audit", actor_id)
    return {"event_name": event_name, "actor_id": actor_id}
