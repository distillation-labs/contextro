"""Generated filler test 024 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_024 import build_workflows_payload_024


def test_generated_payload_024() -> None:
    payload = build_workflows_payload_024("seed")
    assert payload["identifier"].startswith("seed-")
