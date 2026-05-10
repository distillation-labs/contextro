"""Generated filler test 045 for the partners package."""

from __future__ import annotations

from partners.generated.generated_045 import build_partners_payload_045


def test_generated_payload_045() -> None:
    payload = build_partners_payload_045("seed")
    assert payload["identifier"].startswith("seed-")
