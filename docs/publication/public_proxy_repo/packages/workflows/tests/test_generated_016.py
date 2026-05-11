"""Generated filler test 016 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_016 import build_workflows_payload_016


def test_generated_payload_016() -> None:
    payload = build_workflows_payload_016("seed")
    assert payload["identifier"].startswith("seed-")
