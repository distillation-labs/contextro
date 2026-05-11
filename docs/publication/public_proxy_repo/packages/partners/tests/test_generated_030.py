"""Generated filler test 030 for the partners package."""

from __future__ import annotations

from partners.generated.generated_030 import build_partners_payload_030


def test_generated_payload_030() -> None:
    payload = build_partners_payload_030("seed")
    assert payload["identifier"].startswith("seed-")
