"""Generated filler test 045 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_045 import build_workflows_payload_045


def test_generated_payload_045() -> None:
    payload = build_workflows_payload_045("seed")
    assert payload["identifier"].startswith("seed-")
