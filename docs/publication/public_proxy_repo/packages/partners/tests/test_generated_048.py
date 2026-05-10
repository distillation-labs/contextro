"""Generated filler test 048 for the partners package."""

from __future__ import annotations

from partners.generated.generated_048 import build_partners_payload_048


def test_generated_payload_048() -> None:
    payload = build_partners_payload_048("seed")
    assert payload["identifier"].startswith("seed-")
