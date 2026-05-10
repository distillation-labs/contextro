"""Generated filler test 036 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_036 import build_workflows_payload_036


def test_generated_payload_036() -> None:
    payload = build_workflows_payload_036("seed")
    assert payload["identifier"].startswith("seed-")
