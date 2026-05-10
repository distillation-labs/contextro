"""Generated filler test 010 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_010 import build_workflows_payload_010


def test_generated_payload_010() -> None:
    payload = build_workflows_payload_010("seed")
    assert payload["identifier"].startswith("seed-")
