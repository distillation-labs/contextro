"""Generated filler test 005 for the partners package."""

from __future__ import annotations

from partners.generated.generated_005 import build_partners_payload_005


def test_generated_payload_005() -> None:
    payload = build_partners_payload_005("seed")
    assert payload["identifier"].startswith("seed-")
