"""Generated filler test 001 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_001 import build_workflows_payload_001


def test_generated_payload_001() -> None:
    payload = build_workflows_payload_001("seed")
    assert payload["identifier"].startswith("seed-")
