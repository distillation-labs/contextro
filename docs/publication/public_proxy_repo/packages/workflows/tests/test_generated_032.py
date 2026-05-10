"""Generated filler test 032 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_032 import build_workflows_payload_032


def test_generated_payload_032() -> None:
    payload = build_workflows_payload_032("seed")
    assert payload["identifier"].startswith("seed-")
