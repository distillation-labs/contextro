"""Generated filler test 046 for the partners package."""

from __future__ import annotations

from partners.generated.generated_046 import build_partners_payload_046


def test_generated_payload_046() -> None:
    payload = build_partners_payload_046("seed")
    assert payload["identifier"].startswith("seed-")
