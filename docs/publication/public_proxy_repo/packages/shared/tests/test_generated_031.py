"""Generated filler test 031 for the shared package."""

from __future__ import annotations

from shared.generated.generated_031 import build_shared_payload_031


def test_generated_payload_031() -> None:
    payload = build_shared_payload_031("seed")
    assert payload["identifier"].startswith("seed-")
