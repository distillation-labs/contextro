"""Generated filler test 039 for the shared package."""

from __future__ import annotations

from shared.generated.generated_039 import build_shared_payload_039


def test_generated_payload_039() -> None:
    payload = build_shared_payload_039("seed")
    assert payload["identifier"].startswith("seed-")
