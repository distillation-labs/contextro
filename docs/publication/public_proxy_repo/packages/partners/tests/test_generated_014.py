"""Generated filler test 014 for the partners package."""

from __future__ import annotations

from partners.generated.generated_014 import build_partners_payload_014


def test_generated_payload_014() -> None:
    payload = build_partners_payload_014("seed")
    assert payload["identifier"].startswith("seed-")
