"""Generated filler test 023 for the partners package."""

from __future__ import annotations

from partners.generated.generated_023 import build_partners_payload_023


def test_generated_payload_023() -> None:
    payload = build_partners_payload_023("seed")
    assert payload["identifier"].startswith("seed-")
