"""Generated filler test 046 for the shared package."""

from __future__ import annotations

from shared.generated.generated_046 import build_shared_payload_046


def test_generated_payload_046() -> None:
    payload = build_shared_payload_046("seed")
    assert payload["identifier"].startswith("seed-")
