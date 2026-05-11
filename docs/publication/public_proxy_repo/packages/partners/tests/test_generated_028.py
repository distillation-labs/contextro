"""Generated filler test 028 for the partners package."""

from __future__ import annotations

from partners.generated.generated_028 import build_partners_payload_028


def test_generated_payload_028() -> None:
    payload = build_partners_payload_028("seed")
    assert payload["identifier"].startswith("seed-")
