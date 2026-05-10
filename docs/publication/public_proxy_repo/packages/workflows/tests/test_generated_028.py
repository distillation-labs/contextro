"""Generated filler test 028 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_028 import build_workflows_payload_028


def test_generated_payload_028() -> None:
    payload = build_workflows_payload_028("seed")
    assert payload["identifier"].startswith("seed-")
