"""Generated filler test 043 for the partners package."""

from __future__ import annotations

from partners.generated.generated_043 import build_partners_payload_043


def test_generated_payload_043() -> None:
    payload = build_partners_payload_043("seed")
    assert payload["identifier"].startswith("seed-")
