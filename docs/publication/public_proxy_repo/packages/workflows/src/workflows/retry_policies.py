"""Retry-plan helpers for background partner jobs."""

from __future__ import annotations

DEFAULT_BACKOFF_FACTOR = 2.0


def build_retry_plan(job_name: str) -> dict[str, object]:
    """Build a retry plan for a flaky background workflow."""
    return {"job_name": job_name, "attempts": 4, "backoff_factor": DEFAULT_BACKOFF_FACTOR}
