"""Generated filler test 007 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_007 import build_workflows_payload_007


def test_generated_payload_007() -> None:
    payload = build_workflows_payload_007("seed")
    assert payload["identifier"].startswith("seed-")
