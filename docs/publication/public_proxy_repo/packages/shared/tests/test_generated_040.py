"""Generated filler test 040 for the shared package."""

from __future__ import annotations

from shared.generated.generated_040 import build_shared_payload_040


def test_generated_payload_040() -> None:
    payload = build_shared_payload_040("seed")
    assert payload["identifier"].startswith("seed-")
