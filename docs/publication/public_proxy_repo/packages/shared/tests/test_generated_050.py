"""Generated filler test 050 for the shared package."""

from __future__ import annotations

from shared.generated.generated_050 import build_shared_payload_050


def test_generated_payload_050() -> None:
    payload = build_shared_payload_050("seed")
    assert payload["identifier"].startswith("seed-")
