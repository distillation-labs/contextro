"""Digest scheduling for batched partner alerts."""

from __future__ import annotations

from workflows.retry_policies import build_retry_plan

DEFAULT_DIGEST_DELAY_MINUTES = 45


def schedule_digest_delivery(
    account_name: str,
    delay_minutes: int = DEFAULT_DIGEST_DELAY_MINUTES,
) -> dict[str, object]:
    """Schedule digest delivery for batched partner notifications."""
    return {
        "account_name": account_name,
        "delay_minutes": delay_minutes,
        "retry": build_retry_plan("digest_delivery"),
    }
