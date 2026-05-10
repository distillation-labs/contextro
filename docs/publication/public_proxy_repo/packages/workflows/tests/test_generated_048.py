"""Generated filler test 048 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_048 import build_workflows_payload_048


def test_generated_payload_048() -> None:
    payload = build_workflows_payload_048("seed")
    assert payload["identifier"].startswith("seed-")
