"""Generated filler test 036 for the shared package."""

from __future__ import annotations

from shared.generated.generated_036 import build_shared_payload_036


def test_generated_payload_036() -> None:
    payload = build_shared_payload_036("seed")
    assert payload["identifier"].startswith("seed-")
