"""Generated filler test 037 for the shared package."""

from __future__ import annotations

from shared.generated.generated_037 import build_shared_payload_037


def test_generated_payload_037() -> None:
    payload = build_shared_payload_037("seed")
    assert payload["identifier"].startswith("seed-")
