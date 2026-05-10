"""Generated filler test 025 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_025 import build_workflows_payload_025


def test_generated_payload_025() -> None:
    payload = build_workflows_payload_025("seed")
    assert payload["identifier"].startswith("seed-")
