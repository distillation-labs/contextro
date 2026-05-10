"""Generated filler test 002 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_002 import build_workflows_payload_002


def test_generated_payload_002() -> None:
    payload = build_workflows_payload_002("seed")
    assert payload["identifier"].startswith("seed-")
