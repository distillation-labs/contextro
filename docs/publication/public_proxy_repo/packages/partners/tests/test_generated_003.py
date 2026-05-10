"""Generated filler test 003 for the partners package."""

from __future__ import annotations

from partners.generated.generated_003 import build_partners_payload_003


def test_generated_payload_003() -> None:
    payload = build_partners_payload_003("seed")
    assert payload["identifier"].startswith("seed-")
