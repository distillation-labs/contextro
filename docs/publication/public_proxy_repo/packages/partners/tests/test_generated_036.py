"""Generated filler test 036 for the partners package."""

from __future__ import annotations

from partners.generated.generated_036 import build_partners_payload_036


def test_generated_payload_036() -> None:
    payload = build_partners_payload_036("seed")
    assert payload["identifier"].startswith("seed-")
