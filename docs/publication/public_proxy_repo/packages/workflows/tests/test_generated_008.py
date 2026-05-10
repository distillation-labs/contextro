"""Generated filler test 008 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_008 import build_workflows_payload_008


def test_generated_payload_008() -> None:
    payload = build_workflows_payload_008("seed")
    assert payload["identifier"].startswith("seed-")
