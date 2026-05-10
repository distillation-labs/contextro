"""Generated filler test 019 for the partners package."""

from __future__ import annotations

from partners.generated.generated_019 import build_partners_payload_019


def test_generated_payload_019() -> None:
    payload = build_partners_payload_019("seed")
    assert payload["identifier"].startswith("seed-")
