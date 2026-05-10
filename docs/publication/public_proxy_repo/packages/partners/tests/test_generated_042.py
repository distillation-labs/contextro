"""Generated filler test 042 for the partners package."""

from __future__ import annotations

from partners.generated.generated_042 import build_partners_payload_042


def test_generated_payload_042() -> None:
    payload = build_partners_payload_042("seed")
    assert payload["identifier"].startswith("seed-")
