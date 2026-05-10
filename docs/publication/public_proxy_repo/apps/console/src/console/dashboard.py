"""Console dashboard orchestration for partner operators."""

from __future__ import annotations

from catalog.profile_projector import PartnerProfileProjector
from experiments.rollouts import evaluate_rollout_gate
from partners.onboarding import prepare_partner_onboarding_context


def build_partner_dashboard(alias: str, actor_id: str) -> dict[str, object]:
    """Build the operator dashboard for a partner profile review."""
    context = prepare_partner_onboarding_context(alias, actor_id)
    projector = PartnerProfileProjector()
    card = projector.project(alias, context["profile"])
    card["show_rollout_banner"] = evaluate_rollout_gate(actor_id, sample_size=320, bucket=2)
    return {"context": context, "card": card}
