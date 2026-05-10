"""Generated filler test 013 for the partners package."""

from __future__ import annotations

from partners.generated.generated_013 import build_partners_payload_013


def test_generated_payload_013() -> None:
    payload = build_partners_payload_013("seed")
    assert payload["identifier"].startswith("seed-")
