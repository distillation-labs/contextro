"""Generated filler test 023 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_023 import build_workflows_payload_023


def test_generated_payload_023() -> None:
    payload = build_workflows_payload_023("seed")
    assert payload["identifier"].startswith("seed-")
