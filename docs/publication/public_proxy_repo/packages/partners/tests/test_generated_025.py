"""Generated filler test 025 for the partners package."""

from __future__ import annotations

from partners.generated.generated_025 import build_partners_payload_025


def test_generated_payload_025() -> None:
    payload = build_partners_payload_025("seed")
    assert payload["identifier"].startswith("seed-")
