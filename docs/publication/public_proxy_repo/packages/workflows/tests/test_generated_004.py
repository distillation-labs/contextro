"""Generated filler test 004 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_004 import build_workflows_payload_004


def test_generated_payload_004() -> None:
    payload = build_workflows_payload_004("seed")
    assert payload["identifier"].startswith("seed-")
