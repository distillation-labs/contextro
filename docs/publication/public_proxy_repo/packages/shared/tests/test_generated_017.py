"""Generated filler test 017 for the shared package."""

from __future__ import annotations

from shared.generated.generated_017 import build_shared_payload_017


def test_generated_payload_017() -> None:
    payload = build_shared_payload_017("seed")
    assert payload["identifier"].startswith("seed-")
