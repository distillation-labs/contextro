"""Generated filler test 029 for the partners package."""

from __future__ import annotations

from partners.generated.generated_029 import build_partners_payload_029


def test_generated_payload_029() -> None:
    payload = build_partners_payload_029("seed")
    assert payload["identifier"].startswith("seed-")
