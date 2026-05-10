"""Generated filler test 015 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_015 import build_workflows_payload_015


def test_generated_payload_015() -> None:
    payload = build_workflows_payload_015("seed")
    assert payload["identifier"].startswith("seed-")
