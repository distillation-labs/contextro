"""Generated filler test 047 for the shared package."""

from __future__ import annotations

from shared.generated.generated_047 import build_shared_payload_047


def test_generated_payload_047() -> None:
    payload = build_shared_payload_047("seed")
    assert payload["identifier"].startswith("seed-")
