"""Generated filler test 016 for the shared package."""

from __future__ import annotations

from shared.generated.generated_016 import build_shared_payload_016


def test_generated_payload_016() -> None:
    payload = build_shared_payload_016("seed")
    assert payload["identifier"].startswith("seed-")
