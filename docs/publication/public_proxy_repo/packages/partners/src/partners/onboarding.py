"""Context assembly for partner onboarding review workflows."""

from __future__ import annotations

from auth.audit_bridge import emit_auth_audit_event
from catalog.profile_store import load_partner_profile
from shared.context import build_actor_context

from partners.aliases import normalize_partner_alias


def prepare_partner_onboarding_context(
    alias: str, actor_id: str
) -> dict[str, object]:
    """Prepare the dashboard context used before partner onboarding review."""
    normalized = normalize_partner_alias(alias)
    actor_context = build_actor_context(actor_id)
    profile = load_partner_profile(normalized)
    emit_auth_audit_event("partner_onboarding.reviewed", actor_id)
    return {
        "normalized_alias": normalized,
        "actor_context": actor_context,
        "profile": profile,
    }
