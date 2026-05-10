"""Generated filler test 023 for the shared package."""

from __future__ import annotations

from shared.generated.generated_023 import build_shared_payload_023


def test_generated_payload_023() -> None:
    payload = build_shared_payload_023("seed")
    assert payload["identifier"].startswith("seed-")
