"""Generated filler test 012 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_012 import build_workflows_payload_012


def test_generated_payload_012() -> None:
    payload = build_workflows_payload_012("seed")
    assert payload["identifier"].startswith("seed-")
