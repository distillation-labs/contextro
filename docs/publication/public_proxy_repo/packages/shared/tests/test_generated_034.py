"""Generated filler test 034 for the shared package."""

from __future__ import annotations

from shared.generated.generated_034 import build_shared_payload_034


def test_generated_payload_034() -> None:
    payload = build_shared_payload_034("seed")
    assert payload["identifier"].startswith("seed-")
