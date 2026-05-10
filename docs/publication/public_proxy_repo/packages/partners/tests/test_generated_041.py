"""Generated filler test 041 for the partners package."""

from __future__ import annotations

from partners.generated.generated_041 import build_partners_payload_041


def test_generated_payload_041() -> None:
    payload = build_partners_payload_041("seed")
    assert payload["identifier"].startswith("seed-")
