"""Generated filler test 017 for the partners package."""

from __future__ import annotations

from partners.generated.generated_017 import build_partners_payload_017


def test_generated_payload_017() -> None:
    payload = build_partners_payload_017("seed")
    assert payload["identifier"].startswith("seed-")
