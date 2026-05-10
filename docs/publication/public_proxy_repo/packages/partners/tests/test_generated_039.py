"""Generated filler test 039 for the partners package."""

from __future__ import annotations

from partners.generated.generated_039 import build_partners_payload_039


def test_generated_payload_039() -> None:
    payload = build_partners_payload_039("seed")
    assert payload["identifier"].startswith("seed-")
