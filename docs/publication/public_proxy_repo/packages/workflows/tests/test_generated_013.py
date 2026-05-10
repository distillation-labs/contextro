"""Generated filler test 013 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_013 import build_workflows_payload_013


def test_generated_payload_013() -> None:
    payload = build_workflows_payload_013("seed")
    assert payload["identifier"].startswith("seed-")
