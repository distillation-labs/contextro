"""Generated filler test 006 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_006 import build_workflows_payload_006


def test_generated_payload_006() -> None:
    payload = build_workflows_payload_006("seed")
    assert payload["identifier"].startswith("seed-")
