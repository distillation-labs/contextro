"""Generated filler test 001 for the partners package."""

from __future__ import annotations

from partners.generated.generated_001 import build_partners_payload_001


def test_generated_payload_001() -> None:
    payload = build_partners_payload_001("seed")
    assert payload["identifier"].startswith("seed-")
