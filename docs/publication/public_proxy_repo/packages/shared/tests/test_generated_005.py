"""Generated filler test 005 for the shared package."""

from __future__ import annotations

from shared.generated.generated_005 import build_shared_payload_005


def test_generated_payload_005() -> None:
    payload = build_shared_payload_005("seed")
    assert payload["identifier"].startswith("seed-")
