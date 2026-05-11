"""Generated filler test 027 for the partners package."""

from __future__ import annotations

from partners.generated.generated_027 import build_partners_payload_027


def test_generated_payload_027() -> None:
    payload = build_partners_payload_027("seed")
    assert payload["identifier"].startswith("seed-")
