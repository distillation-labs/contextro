"""Generated filler test 040 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_040 import build_workflows_payload_040


def test_generated_payload_040() -> None:
    payload = build_workflows_payload_040("seed")
    assert payload["identifier"].startswith("seed-")
