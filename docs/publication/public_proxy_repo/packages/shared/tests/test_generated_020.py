"""Generated filler test 020 for the shared package."""

from __future__ import annotations

from shared.generated.generated_020 import build_shared_payload_020


def test_generated_payload_020() -> None:
    payload = build_shared_payload_020("seed")
    assert payload["identifier"].startswith("seed-")
