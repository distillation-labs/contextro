"""Generated filler test 004 for the partners package."""

from __future__ import annotations

from partners.generated.generated_004 import build_partners_payload_004


def test_generated_payload_004() -> None:
    payload = build_partners_payload_004("seed")
    assert payload["identifier"].startswith("seed-")
