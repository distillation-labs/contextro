"""Generated filler test 016 for the partners package."""

from __future__ import annotations

from partners.generated.generated_016 import build_partners_payload_016


def test_generated_payload_016() -> None:
    payload = build_partners_payload_016("seed")
    assert payload["identifier"].startswith("seed-")
