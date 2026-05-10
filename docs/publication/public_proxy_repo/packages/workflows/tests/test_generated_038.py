"""Generated filler test 038 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_038 import build_workflows_payload_038


def test_generated_payload_038() -> None:
    payload = build_workflows_payload_038("seed")
    assert payload["identifier"].startswith("seed-")
