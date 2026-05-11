"""Generated filler test 032 for the shared package."""

from __future__ import annotations

from shared.generated.generated_032 import build_shared_payload_032


def test_generated_payload_032() -> None:
    payload = build_shared_payload_032("seed")
    assert payload["identifier"].startswith("seed-")
