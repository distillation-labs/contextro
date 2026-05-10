"""Generated filler test 033 for the partners package."""

from __future__ import annotations

from partners.generated.generated_033 import build_partners_payload_033


def test_generated_payload_033() -> None:
    payload = build_partners_payload_033("seed")
    assert payload["identifier"].startswith("seed-")
