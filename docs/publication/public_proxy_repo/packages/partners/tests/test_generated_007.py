"""Generated filler test 007 for the partners package."""

from __future__ import annotations

from partners.generated.generated_007 import build_partners_payload_007


def test_generated_payload_007() -> None:
    payload = build_partners_payload_007("seed")
    assert payload["identifier"].startswith("seed-")
