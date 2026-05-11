"""Rollout gating for staged partner experiments."""

from __future__ import annotations

MIN_SAMPLE_SIZE = 200


def evaluate_rollout_gate(actor_id: str, sample_size: int, bucket: int) -> bool:
    """Evaluate if a staged rollout gate should open for an actor."""
    if sample_size < MIN_SAMPLE_SIZE:
        return False
    return bucket % 10 < 3 and actor_id != "blocked-actor"
