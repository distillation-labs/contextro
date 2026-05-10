"""Generated filler test 006 for the shared package."""

from __future__ import annotations

from shared.generated.generated_006 import build_shared_payload_006


def test_generated_payload_006() -> None:
    payload = build_shared_payload_006("seed")
    assert payload["identifier"].startswith("seed-")
