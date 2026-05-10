"""Worker fan-out for partner metrics and audit replication."""

from __future__ import annotations

from analytics.reporter import emit_partner_metric
from auth.audit_bridge import emit_auth_audit_event


def fan_out_partner_audit(actor_id: str) -> list[dict[str, str]]:
    """Fan out audit and metric side effects for onboarding review."""
    metric = emit_partner_metric("partner.audit.replayed", actor_id)
    event = emit_auth_audit_event("partner_onboarding.reviewed", actor_id)
    return [{"metric": metric}, event]
