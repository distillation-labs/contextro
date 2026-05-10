"""Generated filler test 008 for the partners package."""

from __future__ import annotations

from partners.generated.generated_008 import build_partners_payload_008


def test_generated_payload_008() -> None:
    payload = build_partners_payload_008("seed")
    assert payload["identifier"].startswith("seed-")
