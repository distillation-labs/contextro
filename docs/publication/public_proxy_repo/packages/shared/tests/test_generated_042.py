"""Generated filler test 042 for the shared package."""

from __future__ import annotations

from shared.generated.generated_042 import build_shared_payload_042


def test_generated_payload_042() -> None:
    payload = build_shared_payload_042("seed")
    assert payload["identifier"].startswith("seed-")
