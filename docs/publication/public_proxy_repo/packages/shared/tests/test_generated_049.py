"""Generated filler test 049 for the shared package."""

from __future__ import annotations

from shared.generated.generated_049 import build_shared_payload_049


def test_generated_payload_049() -> None:
    payload = build_shared_payload_049("seed")
    assert payload["identifier"].startswith("seed-")
