"""Generated filler test 038 for the shared package."""

from __future__ import annotations

from shared.generated.generated_038 import build_shared_payload_038


def test_generated_payload_038() -> None:
    payload = build_shared_payload_038("seed")
    assert payload["identifier"].startswith("seed-")
