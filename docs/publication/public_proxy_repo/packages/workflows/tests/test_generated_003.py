"""Generated filler test 003 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_003 import build_workflows_payload_003


def test_generated_payload_003() -> None:
    payload = build_workflows_payload_003("seed")
    assert payload["identifier"].startswith("seed-")
