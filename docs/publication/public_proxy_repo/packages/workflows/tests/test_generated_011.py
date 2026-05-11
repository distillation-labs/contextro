"""Generated filler test 011 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_011 import build_workflows_payload_011


def test_generated_payload_011() -> None:
    payload = build_workflows_payload_011("seed")
    assert payload["identifier"].startswith("seed-")
