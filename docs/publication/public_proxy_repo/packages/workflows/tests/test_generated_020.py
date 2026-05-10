"""Generated filler test 020 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_020 import build_workflows_payload_020


def test_generated_payload_020() -> None:
    payload = build_workflows_payload_020("seed")
    assert payload["identifier"].startswith("seed-")
