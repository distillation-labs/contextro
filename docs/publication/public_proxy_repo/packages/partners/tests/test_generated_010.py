"""Generated filler test 010 for the partners package."""

from __future__ import annotations

from partners.generated.generated_010 import build_partners_payload_010


def test_generated_payload_010() -> None:
    payload = build_partners_payload_010("seed")
    assert payload["identifier"].startswith("seed-")
