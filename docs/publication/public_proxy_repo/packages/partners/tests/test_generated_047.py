"""Generated filler test 047 for the partners package."""

from __future__ import annotations

from partners.generated.generated_047 import build_partners_payload_047


def test_generated_payload_047() -> None:
    payload = build_partners_payload_047("seed")
    assert payload["identifier"].startswith("seed-")
