"""Generated filler test 030 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_030 import build_workflows_payload_030


def test_generated_payload_030() -> None:
    payload = build_workflows_payload_030("seed")
    assert payload["identifier"].startswith("seed-")
