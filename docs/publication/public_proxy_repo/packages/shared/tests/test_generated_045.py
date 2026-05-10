"""Generated filler test 045 for the shared package."""

from __future__ import annotations

from shared.generated.generated_045 import build_shared_payload_045


def test_generated_payload_045() -> None:
    payload = build_shared_payload_045("seed")
    assert payload["identifier"].startswith("seed-")
