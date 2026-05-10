"""Generated filler test 041 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_041 import build_workflows_payload_041


def test_generated_payload_041() -> None:
    payload = build_workflows_payload_041("seed")
    assert payload["identifier"].startswith("seed-")
