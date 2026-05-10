"""Generated filler test 034 for the partners package."""

from __future__ import annotations

from partners.generated.generated_034 import build_partners_payload_034


def test_generated_payload_034() -> None:
    payload = build_partners_payload_034("seed")
    assert payload["identifier"].startswith("seed-")
