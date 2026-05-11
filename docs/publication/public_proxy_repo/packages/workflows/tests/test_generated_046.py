"""Generated filler test 046 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_046 import build_workflows_payload_046


def test_generated_payload_046() -> None:
    payload = build_workflows_payload_046("seed")
    assert payload["identifier"].startswith("seed-")
