"""Generated filler test 012 for the shared package."""

from __future__ import annotations

from shared.generated.generated_012 import build_shared_payload_012


def test_generated_payload_012() -> None:
    payload = build_shared_payload_012("seed")
    assert payload["identifier"].startswith("seed-")
