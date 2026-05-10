"""Generated filler test 047 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_047 import build_workflows_payload_047


def test_generated_payload_047() -> None:
    payload = build_workflows_payload_047("seed")
    assert payload["identifier"].startswith("seed-")
