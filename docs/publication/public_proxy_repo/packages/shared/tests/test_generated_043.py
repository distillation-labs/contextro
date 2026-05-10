"""Generated filler test 043 for the shared package."""

from __future__ import annotations

from shared.generated.generated_043 import build_shared_payload_043


def test_generated_payload_043() -> None:
    payload = build_shared_payload_043("seed")
    assert payload["identifier"].startswith("seed-")
