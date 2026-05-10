"""Generated filler test 018 for the partners package."""

from __future__ import annotations

from partners.generated.generated_018 import build_partners_payload_018


def test_generated_payload_018() -> None:
    payload = build_partners_payload_018("seed")
    assert payload["identifier"].startswith("seed-")
