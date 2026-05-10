"""Generated filler test 015 for the partners package."""

from __future__ import annotations

from partners.generated.generated_015 import build_partners_payload_015


def test_generated_payload_015() -> None:
    payload = build_partners_payload_015("seed")
    assert payload["identifier"].startswith("seed-")
