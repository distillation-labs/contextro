"""Generated filler test 009 for the partners package."""

from __future__ import annotations

from partners.generated.generated_009 import build_partners_payload_009


def test_generated_payload_009() -> None:
    payload = build_partners_payload_009("seed")
    assert payload["identifier"].startswith("seed-")
