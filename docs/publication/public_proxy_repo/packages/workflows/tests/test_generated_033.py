"""Generated filler test 033 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_033 import build_workflows_payload_033


def test_generated_payload_033() -> None:
    payload = build_workflows_payload_033("seed")
    assert payload["identifier"].startswith("seed-")
