"""Generated filler test 035 for the partners package."""

from __future__ import annotations

from partners.generated.generated_035 import build_partners_payload_035


def test_generated_payload_035() -> None:
    payload = build_partners_payload_035("seed")
    assert payload["identifier"].startswith("seed-")
