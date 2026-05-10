"""Generated filler test 044 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_044 import build_workflows_payload_044


def test_generated_payload_044() -> None:
    payload = build_workflows_payload_044("seed")
    assert payload["identifier"].startswith("seed-")
