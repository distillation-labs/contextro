"""Generated filler test 029 for the shared package."""

from __future__ import annotations

from shared.generated.generated_029 import build_shared_payload_029


def test_generated_payload_029() -> None:
    payload = build_shared_payload_029("seed")
    assert payload["identifier"].startswith("seed-")
