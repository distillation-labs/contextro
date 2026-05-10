"""Generated filler test 049 for the partners package."""

from __future__ import annotations

from partners.generated.generated_049 import build_partners_payload_049


def test_generated_payload_049() -> None:
    payload = build_partners_payload_049("seed")
    assert payload["identifier"].startswith("seed-")
