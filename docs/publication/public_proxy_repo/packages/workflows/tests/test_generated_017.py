"""Generated filler test 017 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_017 import build_workflows_payload_017


def test_generated_payload_017() -> None:
    payload = build_workflows_payload_017("seed")
    assert payload["identifier"].startswith("seed-")
