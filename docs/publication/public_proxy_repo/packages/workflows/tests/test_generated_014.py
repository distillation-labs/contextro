"""Generated filler test 014 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_014 import build_workflows_payload_014


def test_generated_payload_014() -> None:
    payload = build_workflows_payload_014("seed")
    assert payload["identifier"].startswith("seed-")
