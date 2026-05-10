"""Generated filler test 032 for the partners package."""

from __future__ import annotations

from partners.generated.generated_032 import build_partners_payload_032


def test_generated_payload_032() -> None:
    payload = build_partners_payload_032("seed")
    assert payload["identifier"].startswith("seed-")
