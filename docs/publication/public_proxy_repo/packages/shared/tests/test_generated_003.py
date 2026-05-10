"""Generated filler test 003 for the shared package."""

from __future__ import annotations

from shared.generated.generated_003 import build_shared_payload_003


def test_generated_payload_003() -> None:
    payload = build_shared_payload_003("seed")
    assert payload["identifier"].startswith("seed-")
