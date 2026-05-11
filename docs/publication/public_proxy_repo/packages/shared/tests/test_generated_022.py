"""Generated filler test 022 for the shared package."""

from __future__ import annotations

from shared.generated.generated_022 import build_shared_payload_022


def test_generated_payload_022() -> None:
    payload = build_shared_payload_022("seed")
    assert payload["identifier"].startswith("seed-")
