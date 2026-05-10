"""Generated filler test 009 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_009 import build_workflows_payload_009


def test_generated_payload_009() -> None:
    payload = build_workflows_payload_009("seed")
    assert payload["identifier"].startswith("seed-")
