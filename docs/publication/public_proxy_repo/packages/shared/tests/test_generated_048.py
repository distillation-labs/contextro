"""Generated filler test 048 for the shared package."""

from __future__ import annotations

from shared.generated.generated_048 import build_shared_payload_048


def test_generated_payload_048() -> None:
    payload = build_shared_payload_048("seed")
    assert payload["identifier"].startswith("seed-")
