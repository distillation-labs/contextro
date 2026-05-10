"""Generated filler test 050 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_050 import build_workflows_payload_050


def test_generated_payload_050() -> None:
    payload = build_workflows_payload_050("seed")
    assert payload["identifier"].startswith("seed-")
