"""Generated filler test 031 for the partners package."""

from __future__ import annotations

from partners.generated.generated_031 import build_partners_payload_031


def test_generated_payload_031() -> None:
    payload = build_partners_payload_031("seed")
    assert payload["identifier"].startswith("seed-")
