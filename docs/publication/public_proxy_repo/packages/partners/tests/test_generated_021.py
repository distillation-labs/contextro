"""Generated filler test 021 for the partners package."""

from __future__ import annotations

from partners.generated.generated_021 import build_partners_payload_021


def test_generated_payload_021() -> None:
    payload = build_partners_payload_021("seed")
    assert payload["identifier"].startswith("seed-")
