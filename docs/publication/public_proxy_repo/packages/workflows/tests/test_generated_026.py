"""Generated filler test 026 for the workflows package."""

from __future__ import annotations

from workflows.generated.generated_026 import build_workflows_payload_026


def test_generated_payload_026() -> None:
    payload = build_workflows_payload_026("seed")
    assert payload["identifier"].startswith("seed-")
