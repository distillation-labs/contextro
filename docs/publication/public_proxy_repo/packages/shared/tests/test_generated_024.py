"""Generated filler test 024 for the shared package."""

from __future__ import annotations

from shared.generated.generated_024 import build_shared_payload_024


def test_generated_payload_024() -> None:
    payload = build_shared_payload_024("seed")
    assert payload["identifier"].startswith("seed-")
