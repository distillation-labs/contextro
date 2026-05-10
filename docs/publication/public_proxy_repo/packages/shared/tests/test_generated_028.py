"""Generated filler test 028 for the shared package."""

from __future__ import annotations

from shared.generated.generated_028 import build_shared_payload_028


def test_generated_payload_028() -> None:
    payload = build_shared_payload_028("seed")
    assert payload["identifier"].startswith("seed-")
