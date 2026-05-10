"""Generated filler test 034 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_034 import build_workflows_payload_034


def test_generated_payload_034() -> None:
    payload = build_workflows_payload_034("seed")
    assert payload["identifier"].startswith("seed-")
