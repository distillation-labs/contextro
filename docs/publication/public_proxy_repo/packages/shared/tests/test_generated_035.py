"""Generated filler test 035 for the shared package."""

from __future__ import annotations

from shared.generated.generated_035 import build_shared_payload_035


def test_generated_payload_035() -> None:
    payload = build_shared_payload_035("seed")
    assert payload["identifier"].startswith("seed-")
