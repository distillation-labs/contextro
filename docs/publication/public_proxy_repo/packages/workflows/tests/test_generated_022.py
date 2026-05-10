"""Generated filler test 022 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_022 import build_workflows_payload_022


def test_generated_payload_022() -> None:
    payload = build_workflows_payload_022("seed")
    assert payload["identifier"].startswith("seed-")
