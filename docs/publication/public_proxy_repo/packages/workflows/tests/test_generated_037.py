"""Generated filler test 037 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_037 import build_workflows_payload_037


def test_generated_payload_037() -> None:
    payload = build_workflows_payload_037("seed")
    assert payload["identifier"].startswith("seed-")
