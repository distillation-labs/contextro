"""Generated filler test 035 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_035 import build_workflows_payload_035


def test_generated_payload_035() -> None:
    payload = build_workflows_payload_035("seed")
    assert payload["identifier"].startswith("seed-")
