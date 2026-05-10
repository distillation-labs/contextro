"""Generated filler test 037 for the partners package."""

from __future__ import annotations

from partners.generated.generated_037 import build_partners_payload_037


def test_generated_payload_037() -> None:
    payload = build_partners_payload_037("seed")
    assert payload["identifier"].startswith("seed-")
