"""Generated filler test 014 for the shared package."""

from __future__ import annotations

from shared.generated.generated_014 import build_shared_payload_014


def test_generated_payload_014() -> None:
    payload = build_shared_payload_014("seed")
    assert payload["identifier"].startswith("seed-")
