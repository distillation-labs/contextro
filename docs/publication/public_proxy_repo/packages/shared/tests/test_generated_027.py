"""Generated filler test 027 for the shared package."""

from __future__ import annotations

from shared.generated.generated_027 import build_shared_payload_027


def test_generated_payload_027() -> None:
    payload = build_shared_payload_027("seed")
    assert payload["identifier"].startswith("seed-")
