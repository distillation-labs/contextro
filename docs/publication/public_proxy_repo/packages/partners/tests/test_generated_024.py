"""Generated filler test 024 for the partners package."""

from __future__ import annotations

from partners.generated.generated_024 import build_partners_payload_024


def test_generated_payload_024() -> None:
    payload = build_partners_payload_024("seed")
    assert payload["identifier"].startswith("seed-")
