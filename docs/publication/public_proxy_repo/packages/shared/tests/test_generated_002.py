"""Generated filler test 002 for the shared package."""

from __future__ import annotations

from shared.generated.generated_002 import build_shared_payload_002


def test_generated_payload_002() -> None:
    payload = build_shared_payload_002("seed")
    assert payload["identifier"].startswith("seed-")
