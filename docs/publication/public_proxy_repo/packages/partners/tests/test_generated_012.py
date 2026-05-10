"""Generated filler test 012 for the partners package."""

from __future__ import annotations

from partners.generated.generated_012 import build_partners_payload_012


def test_generated_payload_012() -> None:
    payload = build_partners_payload_012("seed")
    assert payload["identifier"].startswith("seed-")
