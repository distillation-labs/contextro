"""Generated filler test 020 for the partners package."""

from __future__ import annotations

from partners.generated.generated_020 import build_partners_payload_020


def test_generated_payload_020() -> None:
    payload = build_partners_payload_020("seed")
    assert payload["identifier"].startswith("seed-")
