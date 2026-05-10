"""Generated filler test 031 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_031 import build_workflows_payload_031


def test_generated_payload_031() -> None:
    payload = build_workflows_payload_031("seed")
    assert payload["identifier"].startswith("seed-")
