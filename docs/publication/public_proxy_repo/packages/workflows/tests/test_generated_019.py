"""Generated filler test 019 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_019 import build_workflows_payload_019


def test_generated_payload_019() -> None:
    payload = build_workflows_payload_019("seed")
    assert payload["identifier"].startswith("seed-")
