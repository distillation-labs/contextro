"""Generated filler test 022 for the partners package."""

from __future__ import annotations

from partners.generated.generated_022 import build_partners_payload_022


def test_generated_payload_022() -> None:
    payload = build_partners_payload_022("seed")
    assert payload["identifier"].startswith("seed-")
