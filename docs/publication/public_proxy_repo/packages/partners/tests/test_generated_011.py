"""Generated filler test 011 for the partners package."""

from __future__ import annotations

from partners.generated.generated_011 import build_partners_payload_011


def test_generated_payload_011() -> None:
    payload = build_partners_payload_011("seed")
    assert payload["identifier"].startswith("seed-")
