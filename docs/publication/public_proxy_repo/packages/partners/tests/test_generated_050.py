"""Generated filler test 050 for the partners package."""

from __future__ import annotations

from partners.generated.generated_050 import build_partners_payload_050


def test_generated_payload_050() -> None:
    payload = build_partners_payload_050("seed")
    assert payload["identifier"].startswith("seed-")
