"""Generated filler test 013 for the shared package."""

from __future__ import annotations

from shared.generated.generated_013 import build_shared_payload_013


def test_generated_payload_013() -> None:
    payload = build_shared_payload_013("seed")
    assert payload["identifier"].startswith("seed-")
