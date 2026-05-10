"""Generated filler test 043 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_043 import build_workflows_payload_043


def test_generated_payload_043() -> None:
    payload = build_workflows_payload_043("seed")
    assert payload["identifier"].startswith("seed-")
